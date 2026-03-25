/// DDIM / Euler-Ancestral scheduler.
///
/// References:
///   DDIM:  Song et al. 2020 — https://arxiv.org/abs/2010.02502
///   Euler: Karras et al. 2022 — https://arxiv.org/abs/2206.00364
///
/// Controlled by `eta`:
///   eta = 0.0  →  deterministic DDIM (equivalent to Euler, same math)
///   eta = 1.0  →  Euler Ancestral — stochastic noise added each step
///   0 < eta < 1 →  partially stochastic DDIM variant
///
/// Key design choices:
///   - scaled-linear beta schedule (SD default, "squaredcos" variant)
///   - timesteps descend from T-1 → 0 (high noise to low noise)
///   - `add_noise` supports Hires Fix (latent-space img2img)

pub struct DdimScheduler {
    /// ᾱ_t for t in 0..NUM_TRAIN_TIMESTEPS
    alphas_cumprod: Vec<f64>,
    /// Inference timesteps in descending order, e.g. [999, 979, …, 0]
    timesteps: Vec<usize>,
    /// Stochasticity level: 0=deterministic (DDIM), 1=Euler Ancestral
    pub eta: f64,
}

const NUM_TRAIN_TIMESTEPS: usize = 1000;
const BETA_START: f64 = 0.00085;
const BETA_END: f64 = 0.012;

impl DdimScheduler {
    /// Deterministic DDIM scheduler (eta=0).
    pub fn new(num_inference_steps: usize) -> Self {
        Self::with_eta(num_inference_steps, 0.0)
    }

    /// Scheduler with configurable stochasticity.
    /// `eta=0` → DDIM, `eta=1` → Euler Ancestral.
    pub fn with_eta(num_inference_steps: usize, eta: f64) -> Self {
        let alphas_cumprod = build_alphas_cumprod();
        let step_ratio = NUM_TRAIN_TIMESTEPS / num_inference_steps.max(1);
        let timesteps: Vec<usize> = (0..num_inference_steps)
            .map(|i| (NUM_TRAIN_TIMESTEPS - 1).saturating_sub(i * step_ratio))
            .collect();
        Self {
            alphas_cumprod,
            timesteps,
            eta,
        }
    }

    /// Scheduler for Hires Fix (img2img-style): only denoises the portion of the
    /// schedule at or below the timestep corresponding to `strength`.
    ///
    /// `strength=0.5` means we start halfway through the noise schedule — the
    /// latents are noisified to that level and then denoised back from there.
    pub fn for_img2img(num_steps: usize, strength: f64, eta: f64) -> Self {
        let alphas_cumprod = build_alphas_cumprod();
        let step_ratio = NUM_TRAIN_TIMESTEPS / num_steps.max(1);
        let all_timesteps: Vec<usize> = (0..num_steps)
            .map(|i| (NUM_TRAIN_TIMESTEPS - 1).saturating_sub(i * step_ratio))
            .collect();

        // Keep only the timesteps that fall within the noised region.
        let start_t =
            ((strength * (NUM_TRAIN_TIMESTEPS - 1) as f64) as usize).min(NUM_TRAIN_TIMESTEPS - 1);
        let timesteps: Vec<usize> = all_timesteps
            .into_iter()
            .filter(|&t| t <= start_t)
            .collect();

        Self {
            alphas_cumprod,
            timesteps,
            eta,
        }
    }

    /// Inference timesteps (descending, length == num_inference_steps).
    pub fn timesteps(&self) -> &[usize] {
        &self.timesteps
    }

    /// Perform one scheduler step.
    ///
    /// `latents`    — flat slice [C·H·W]
    /// `noise_pred` — same shape, UNet output after CFG scaling
    /// `step_idx`   — index into `self.timesteps`
    /// `noise`      — required when `eta > 0` (Euler Ancestral random component)
    pub fn step(
        &self,
        latents: &[f32],
        noise_pred: &[f32],
        step_idx: usize,
        noise: Option<&[f32]>,
    ) -> Vec<f32> {
        assert_eq!(latents.len(), noise_pred.len());

        let t = self.timesteps[step_idx];
        let t_prev = if step_idx + 1 < self.timesteps.len() {
            self.timesteps[step_idx + 1]
        } else {
            0
        };

        let abar_t = self.alphas_cumprod[t];
        let abar_t_prev = self.alphas_cumprod[t_prev];

        let sqrt_abar_prev = abar_t_prev.sqrt();
        let sqrt_one_minus_abar_t = (1.0 - abar_t).sqrt();
        let sqrt_abar_t = abar_t.sqrt();

        // σ_t = eta · √((1−ᾱ_{t-1})/(1−ᾱ_t)) · √(1 − ᾱ_t/ᾱ_{t-1})
        let sigma_t = if self.eta > 0.0 {
            self.eta
                * ((1.0 - abar_t_prev) / (1.0 - abar_t)).sqrt()
                * (1.0 - abar_t / abar_t_prev).sqrt()
        } else {
            0.0
        };

        // Coefficient for the "direction toward x_t" term — ensures variance budget.
        let dir_coeff = (1.0 - abar_t_prev - sigma_t * sigma_t).max(0.0).sqrt();

        latents
            .iter()
            .zip(noise_pred.iter())
            .enumerate()
            .map(|(i, (&x_t, &eps))| {
                let x_t = x_t as f64;
                let eps = eps as f64;

                // Predicted clean image: x̂_0 = (x_t − √(1−ᾱ_t)·ε) / √ᾱ_t
                let pred_x0 = (x_t - sqrt_one_minus_abar_t * eps) / sqrt_abar_t;

                // Direction pointing toward x_t
                let dir_xt = dir_coeff * eps;

                // Stochastic component (only when eta > 0)
                let stoch = if sigma_t > 0.0 {
                    sigma_t * noise.map_or(0.0, |n| n[i] as f64)
                } else {
                    0.0
                };

                (sqrt_abar_prev * pred_x0 + dir_xt + stoch) as f32
            })
            .collect()
    }

    /// Noisify `latents` to the noise level corresponding to `timestep`.
    ///
    /// Used by Hires Fix to add noise to upscaled latents before re-denoising:
    ///   x_t = √ᾱ_t · x_0 + √(1−ᾱ_t) · ε
    pub fn add_noise(&self, latents: &[f32], noise: &[f32], timestep: usize) -> Vec<f32> {
        let abar = self.alphas_cumprod[timestep.min(self.alphas_cumprod.len() - 1)];
        let sqrt_abar = abar.sqrt();
        let sqrt_one_minus_abar = (1.0 - abar).sqrt();
        latents
            .iter()
            .zip(noise.iter())
            .map(|(&x, &n)| (sqrt_abar * x as f64 + sqrt_one_minus_abar * n as f64) as f32)
            .collect()
    }

    /// Starting timestep for an img2img pass with a given `strength` (0–1).
    /// This is the training timestep index at which noise is added.
    pub fn start_timestep(strength: f64) -> usize {
        ((strength * (NUM_TRAIN_TIMESTEPS - 1) as f64) as usize).min(NUM_TRAIN_TIMESTEPS - 1)
    }

    /// Scheduler with Karras noise-schedule spacing (for DPM++ 2M Karras).
    ///
    /// Instead of evenly-spaced timesteps, Karras et al. 2022 use:
    ///   σ_i = (σ_max^(1/ρ) + i/(n-1)·(σ_min^(1/ρ) − σ_max^(1/ρ)))^ρ   (ρ=7)
    /// Sigmas run from highest (most noisy) to lowest (least noisy).
    pub fn with_karras(num_inference_steps: usize) -> Self {
        let alphas_cumprod = build_alphas_cumprod();
        let sigma_max = sigma_of(alphas_cumprod[NUM_TRAIN_TIMESTEPS - 1]);
        let sigma_min = sigma_of(alphas_cumprod[0]).max(1e-8);
        let rho = 7.0_f64;
        let n   = num_inference_steps.max(2);

        let karras_sigmas: Vec<f64> = (0..n)
            .map(|i| {
                let frac = i as f64 / (n - 1) as f64;
                let inv  = sigma_max.powf(1.0 / rho)
                    + frac * (sigma_min.powf(1.0 / rho) - sigma_max.powf(1.0 / rho));
                inv.powf(rho)
            })
            .collect();

        let timesteps: Vec<usize> = karras_sigmas.iter()
            .map(|&s| sigma_to_t(s, &alphas_cumprod))
            .collect();

        Self { alphas_cumprod, timesteps, eta: 0.0 }
    }

    /// DPM++ 2M single step (second-order multistep method).
    ///
    /// Reference: Lu et al. 2022, DPM-Solver++ §3.2
    ///
    /// `prev_state` = `Some((prev_denoised, prev_h))` for step > 0 (enables 2nd-order
    /// correction); `None` for the first step (falls back to 1st-order).
    ///
    /// Returns `(new_latents, denoised_current, h_current)`.
    /// The caller stores `(denoised_current, h_current)` and passes it as `prev_state` next step.
    pub fn step_dpm2m(
        &self,
        latents:    &[f32],
        noise_pred: &[f32],
        step_idx:   usize,
        prev_state: Option<(&[f32], f64)>,  // (prev_denoised, prev_h)
    ) -> (Vec<f32>, Vec<f32>, f64) {
        assert_eq!(latents.len(), noise_pred.len());

        let t      = self.timesteps[step_idx];
        let t_prev = if step_idx + 1 < self.timesteps.len() {
            self.timesteps[step_idx + 1]
        } else {
            0
        };

        let abar_t    = self.alphas_cumprod[t];
        let abar_prev = self.alphas_cumprod[t_prev];

        let alpha_t    = abar_t.sqrt();
        let sigma_t    = (1.0 - abar_t).sqrt();
        let alpha_prev = abar_prev.sqrt();
        let sigma_prev = (1.0 - abar_prev).sqrt();

        // λ_t = 0.5·ln(ᾱ_t / (1−ᾱ_t))
        let lambda_t    = 0.5 * (abar_t    / (1.0 - abar_t   )).ln();
        let lambda_prev = 0.5 * (abar_prev / (1.0 - abar_prev)).ln();

        // h = λ_prev − λ_t  (positive: moving from noisy → clean)
        let h = lambda_prev - lambda_t;

        // Predicted clean image: D_t = (x_t − σ_t·ε) / α_t
        let denoised: Vec<f64> = latents.iter().zip(noise_pred.iter())
            .map(|(&x, &e)| (x as f64 - sigma_t * e as f64) / alpha_t)
            .collect();

        // 2nd-order correction when prev step is available; otherwise 1st-order
        let d_hat: Vec<f64> = if let Some((prev_denoised, prev_h)) = prev_state {
            let r = prev_h / h;
            denoised.iter().zip(prev_denoised.iter())
                .map(|(&d_cur, &d_prev)| {
                    (1.0 + 1.0 / (2.0 * r)) * d_cur - (1.0 / (2.0 * r)) * d_prev as f64
                })
                .collect()
        } else {
            denoised.clone()
        };

        // x_prev = (σ_prev/σ_t)·x_t − α_prev·(exp(−h)−1)·D̂
        let expm1_neg_h = (-h).exp() - 1.0;  // negative value
        let new_latents: Vec<f32> = latents.iter().zip(d_hat.iter())
            .map(|(&x_t, &d)| {
                ((sigma_prev / sigma_t) * x_t as f64 - alpha_prev * expm1_neg_h * d) as f32
            })
            .collect();

        let denoised_f32: Vec<f32> = denoised.iter().map(|&d| d as f32).collect();
        (new_latents, denoised_f32, h)
    }

    /// Sigma to multiply the initial random noise by (always 1.0 for DDIM/Euler).
    #[allow(dead_code)]
    pub fn init_noise_sigma(&self) -> f32 {
        1.0
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// σ_t = √((1−ᾱ_t)/ᾱ_t)
fn sigma_of(abar: f64) -> f64 {
    ((1.0 - abar) / abar).sqrt()
}

/// Find the training timestep index t whose σ_t is closest to `target_sigma`.
fn sigma_to_t(target_sigma: f64, alphas_cumprod: &[f64]) -> usize {
    alphas_cumprod
        .iter()
        .enumerate()
        .min_by(|(_, &a_i), (_, &a_j)| {
            let d_i = (sigma_of(a_i) - target_sigma).abs();
            let d_j = (sigma_of(a_j) - target_sigma).abs();
            d_i.partial_cmp(&d_j).unwrap()
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn build_alphas_cumprod() -> Vec<f64> {
    let betas: Vec<f64> = (0..NUM_TRAIN_TIMESTEPS)
        .map(|i| {
            let t = i as f64 / (NUM_TRAIN_TIMESTEPS - 1) as f64;
            let b = BETA_START.sqrt() + t * (BETA_END.sqrt() - BETA_START.sqrt());
            b * b
        })
        .collect();

    let mut alphas_cumprod = Vec::with_capacity(NUM_TRAIN_TIMESTEPS);
    let mut running = 1.0_f64;
    for &b in &betas {
        running *= 1.0 - b;
        alphas_cumprod.push(running);
    }
    alphas_cumprod
}
