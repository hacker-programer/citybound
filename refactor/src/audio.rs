// Sistema de Audio Básico
//
// TÉCNICA COMÚN #6 (juegos): Descompresión de Audio a PCM/WAV
// Generamos tonos de audio proceduralmente durante la carga,
// almacenándolos como buffers PCM crudos en RAM.
// Cero decodificación en tiempo de ejecución.
//
// Como el proyecto original no tiene assets de audio,
// generamos tonos sintéticos mínimos (sine waves, noise)
// usando LUTs trigonométricas precalculadas [TC#5].
//
// Nota: Este es un placeholder de audio. Para audio real,
// se necesitaría cpal o rodio como dependencia.

use crate::luts;

// ---------------------------------------------------------------------------
// TIPOS DE AUDIO
// ---------------------------------------------------------------------------

/// Buffer de audio PCM mono, f32, sample rate 44100 Hz
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Efectos de sonido pre-generados
pub struct SoundEffects {
    /// Click UI
    pub click: AudioBuffer,
    /// Construcción completada
    pub build_complete: AudioBuffer,
    /// Bocina de coche
    pub car_horn: AudioBuffer,
    /// Ambiente de ciudad (loop)
    pub city_ambient: AudioBuffer,
}

// ---------------------------------------------------------------------------
// GENERACIÓN DE TONOS
// ---------------------------------------------------------------------------

impl AudioBuffer {
    /// Crea un buffer de silencio
    pub fn silence(duration_secs: f32, sample_rate: u32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        AudioBuffer {
            samples: vec![0.0_f32; num_samples],
            sample_rate,
        }
    }

    /// Genera un tono sinusoidal puro
    pub fn sine_wave(frequency: f32, duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            // [TC#5]: Usar LUT trigonométrica para seno
            let sample = luts::sin_fast(2.0 * std::f32::consts::PI * frequency * t) * volume;
            samples.push(sample);
        }

        // Aplicar fade out para evitar click
        let fade_samples = (0.01 * sample_rate as f32) as usize; // 10ms fade
        if fade_samples > 0 && fade_samples < num_samples {
            for i in (num_samples - fade_samples)..num_samples {
                let fade = 1.0 - (i - (num_samples - fade_samples)) as f32 / fade_samples as f32;
                samples[i] *= fade;
            }
        }

        AudioBuffer {
            samples,
            sample_rate,
        }
    }

    /// Genera un tono de square wave (más audible en speakers malos)
    pub fn square_wave(frequency: f32, duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let period = sample_rate as f32 / frequency;

        for i in 0..num_samples {
            let phase = (i as f32 % period) / period;
            let sample = if phase < 0.5 { volume } else { -volume };
            samples.push(sample);
        }

        AudioBuffer {
            samples,
            sample_rate,
        }
    }

    /// Genera ruido blanco (para ambiente de ciudad)
    pub fn white_noise(duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        // [TC#22]: Usar RNG rápido para ruido
        let mut state: u64 = 42;
        for _ in 0..num_samples {
            state = state.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^= z >> 31;
            let sample = ((z as f32) / (u64::MAX as f32 + 1.0) * 2.0 - 1.0) * volume;
            samples.push(sample);
        }

        AudioBuffer {
            samples,
            sample_rate,
        }
    }
}

// ---------------------------------------------------------------------------
// GENERADOR DE EFECTOS
// ---------------------------------------------------------------------------

impl SoundEffects {
    /// Genera todos los efectos de sonido durante la carga
    pub fn generate_all() -> Self {
        let sample_rate: u32 = 44100;

        SoundEffects {
            // Click UI: tono corto de 440Hz, 50ms
            click: AudioBuffer::sine_wave(440.0, 0.05, sample_rate, 0.3),

            // Construcción: tono ascendente 200→800Hz, 500ms
            build_complete: {
                let num_samples = (0.5 * sample_rate as f32) as usize;
                let mut samples = Vec::with_capacity(num_samples);
                for i in 0..num_samples {
                    let t = i as f32 / sample_rate as f32;
                    let freq = 200.0 + 600.0 * (t / 0.5);
                    let sample = luts::sin_fast(2.0 * std::f32::consts::PI * freq * t) * 0.4;
                    samples.push(sample);
                }
                AudioBuffer { samples, sample_rate }
            },

            // Bocina: square wave 300Hz, 200ms
            car_horn: AudioBuffer::square_wave(300.0, 0.2, sample_rate, 0.5),

            // Ambiente ciudad: ruido blanco filtrado, 2 segundos (loop)
            city_ambient: AudioBuffer::white_noise(2.0, sample_rate, 0.05),
        }
    }
}

// ---------------------------------------------------------------------------
// REPRODUCTOR DE AUDIO (placeholder)
// ---------------------------------------------------------------------------

/// Estado del reproductor de audio
pub struct AudioPlayer {
    pub effects: SoundEffects,
    /// Volumen master (0.0 - 1.0)
    pub master_volume: f32,
    /// Muted
    pub muted: bool,
}

impl AudioPlayer {
    /// Inicializa el sistema de audio durante la carga
    pub fn init() -> Self {
        AudioPlayer {
            effects: SoundEffects::generate_all(),
            master_volume: 0.7,
            muted: false,
        }
    }

    /// Reproduce un efecto (placeholder - en una implementación real,
    /// enviaría el buffer al thread de audio)
    pub fn play_click(&self) {
        // Placeholder: en producción, enviar self.effects.click al mixer
    }

    pub fn play_build_complete(&self) {
        // Placeholder
    }

    pub fn play_car_horn(&self) {
        // Placeholder
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_wave_not_empty() {
        crate::luts::init_trig_luts();
        let buf = AudioBuffer::sine_wave(440.0, 0.1, 44100, 0.5);
        assert!(!buf.samples.is_empty());
        assert_eq!(buf.sample_rate, 44100);
        assert_eq!(buf.samples.len(), 4410); // 0.1s * 44100
    }

    #[test]
    fn test_sine_wave_range() {
        crate::luts::init_trig_luts();
        let buf = AudioBuffer::sine_wave(440.0, 1.0, 44100, 0.5);
        for sample in &buf.samples {
            assert!(sample.abs() <= 0.5 + 0.01, "Sample fuera de rango: {}", sample);
        }
    }

    #[test]
    fn test_square_wave() {
        let buf = AudioBuffer::square_wave(100.0, 0.2, 44100, 0.8);
        assert!(!buf.samples.is_empty());
        // Verificar que hay valores positivos y negativos
        let has_pos = buf.samples.iter().any(|&s| s > 0.0);
        let has_neg = buf.samples.iter().any(|&s| s < 0.0);
        assert!(has_pos && has_neg, "Square wave debe tener valores ±");
    }

    #[test]
    fn test_white_noise() {
        let buf = AudioBuffer::white_noise(0.5, 44100, 0.3);
        assert_eq!(buf.samples.len(), 22050);

        // Verificar que no es todo igual (es ruido)
        let first = buf.samples[0];
        let different = buf.samples.iter().skip(100).any(|&s| (s - first).abs() > 0.01);
        assert!(different, "White noise debe tener variación");
    }

    #[test]
    fn test_sound_effects_generation() {
        crate::luts::init_trig_luts();
        let fx = SoundEffects::generate_all();

        assert!(!fx.click.samples.is_empty());
        assert!(!fx.build_complete.samples.is_empty());
        assert!(!fx.car_horn.samples.is_empty());
        assert!(!fx.city_ambient.samples.is_empty());
    }

    #[test]
    fn test_audio_player_init() {
        crate::luts::init_trig_luts();
        let player = AudioPlayer::init();
        assert_eq!(player.master_volume, 0.7);
        assert!(!player.muted);
    }

    #[test]
    fn test_silence() {
        let buf = AudioBuffer::silence(0.1, 44100);
        assert_eq!(buf.samples.len(), 4410);
        assert!(buf.samples.iter().all(|&s| s == 0.0));
    }
}
