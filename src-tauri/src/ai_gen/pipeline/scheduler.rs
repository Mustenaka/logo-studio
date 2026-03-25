/// DDIM (Denoising Diffusion Implicit Models) scheduler.
///
/// Reference: Song et al. 2020 — https://arxiv.org/abs/2010.02502
///
/// This is a deterministic (eta=0) DDIM implementation compatible with
/// Stable Diffusion 1.5 and SDXL. It produces the same output as the
/// HuggingFace `diffusers` DDIMScheduler with default settings.
///
/// Key design choices:
///   - scaled-linear beta schedule (SD default, a.k.a. "squaredcos" variant)
///   - eta = 0 → fully deterministic, no extra noise added
///   - timesteps run from T-1 → 0 (high noise to low noise)

pub struct DdimScheduler {
    /// ᾱ_t for t in 0..NUM_TRAIN_TIMESTEPS
    alphas_cumprod: Vec<f64>,
    /// Inference timesteps in descending order, e.g. [999, 979, …, 19, 0]
    timesteps: Vec<usize>,
}

const NUM_TRAIN_TIMESTEPS: usize = 1000;
const BETA_START: f64 = 0.00085;
const BETA_END: f64 = 0.012;

impl DdimScheduler {
    /// Build a scheduler for `num_inference_steps` denoising steps.
    pub fn new(num_inference_steps: usize) -> Self {
        // Scaled-linear (squaredcos) beta schedule used by SD
        let betas: Vec<f64> = (0..NUM_TRAIN_TIMESTEPS)
            .map(|i| {
                let t = i as f64 / (NUM_TRAIN_TIMESTEPS - 1) as f64;
                let b = BETA_START.sqrt() + t * (BETA_END.sqrt() - BETA_START.sqrt());
                b * b
            })
            .collect();

        // Cumulative product: ᾱ_t = ∏_{s=0}^{t} (1 − β_s)
        let mut alphas_cumprod = Vec::with_capacity(NUM_TRAIN_TIMESTEPS);
        let mut running = 1.0_f64;
        for &b in &betas {
            running *= 1.0 - b;
            alphas_cumprod.push(running);
        }

        // Select `num_inference_steps` evenly-spaced timesteps in descending order.
        // Matches diffusers: step_ratio = num_train / num_inference
        let step_ratio = NUM_TRAIN_TIMESTEPS / num_inference_steps;
        let timesteps: Vec<usize> = (0..num_inference_steps)
            .map(|i| {
                // 999, 999 - step_ratio, …, step_ratio, 0
                let t = (NUM_TRAIN_TIMESTEPS - 1).saturating_sub(i * step_ratio);
                t
            })
            .collect();

        Self {
            alphas_cumprod,
            timesteps,
        }
    }

    /// Inference timesteps (descending, length == num_inference_steps).
    pub fn timesteps(&self) -> &[usize] {
        &self.timesteps
    }

    /// Perform one DDIM step.
    ///
    /// Given the current latents `x_t` and the UNet noise prediction `ε_θ`
    /// at timestep index `step_idx`, returns the latents for the previous
    /// timestep `x_{t-1}`.
    ///
    /// `latents`    — flat slice, shape [1·C·H·W]
    /// `noise_pred` — same shape, UNet output (after CFG scaling)
    /// `step_idx`   — index into `self.timesteps`
    pub fn step(
        &self,
        latents: &[f32],
        noise_pred: &[f32],
        step_idx: usize,
    ) -> Vec<f32> {
        assert_eq!(latents.len(), noise_pred.len());

        let t      = self.timesteps[step_idx];
        let t_prev = if step_idx + 1 < self.timesteps.len() {
            self.timesteps[step_idx + 1]
        } else {
            0
        };

        let alpha_bar_t      = self.alphas_cumprod[t];
        let alpha_bar_t_prev = self.alphas_cumprod[t_prev];

        // eta = 0 → σ_t = 0 (fully deterministic DDIM)
        let sqrt_alpha_prev = alpha_bar_t_prev.sqrt();
        let sqrt_one_minus_alpha_prev = (1.0 - alpha_bar_t_prev).sqrt();
        let sqrt_one_minus_alpha_t = (1.0 - alpha_bar_t).sqrt();
        let sqrt_alpha_t = alpha_bar_t.sqrt();

        latents
            .iter()
            .zip(noise_pred.iter())
            .map(|(&x_t, &eps)| {
                let x_t = x_t as f64;
                let eps = eps as f64;

                // Predicted clean image x̂_0 = (x_t − √(1−ᾱ_t)·ε) / √ᾱ_t
                let pred_x0 = (x_t - sqrt_one_minus_alpha_t * eps) / sqrt_alpha_t;

                // Direction pointing toward x_t
                let dir_xt = sqrt_one_minus_alpha_prev * eps;

                // x_{t-1}
                (sqrt_alpha_prev * pred_x0 + dir_xt) as f32
            })
            .collect()
    }

    /// Sigma to multiply the initial random noise by (always 1.0 for DDIM).
    #[allow(dead_code)]
    pub fn init_noise_sigma(&self) -> f32 {
        1.0
    }
}
