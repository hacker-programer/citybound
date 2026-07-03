// Sistema de Audio v0.10 [FASE 7 — Audio real con cpal]
//
// Reproduce tonos procedurales usando cpal (Cross-Platform Audio Library).
// Los buffers se generan durante la carga (sine waves, square waves, ruido).
// Cero decodificación en tiempo de ejecución.
//
// TÉCNICAS:
// [TC#6]  Descompresión de Audio a PCM/WAV (pre-generado)
// [TC#5]  LUTs trigonométricas para síntesis
// [TC#22] RNG pool para ruido
//
// Canales de audio:
// - Click UI (sine 440Hz, 50ms)
// - Construcción (sweep 200→800Hz, 500ms)
// - Bocina (square 300Hz, 200ms)
// - Ambiente ciudad (ruido blanco)

use crate::luts;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Buffer de audio pre-generado
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Efectos de sonido pre-generados
pub struct SoundEffects {
    pub click: AudioBuffer,
    pub build_complete: AudioBuffer,
    pub car_horn: AudioBuffer,
    pub city_ambient: AudioBuffer,
}

/// Evento de audio a reproducir
#[derive(Clone, Debug)]
pub enum AudioEvent {
    Click,
    BuildComplete,
    CarHorn,
    AmbientOn,
    AmbientOff,
}

/// Motor de audio que gestiona reproducción
pub struct AudioEngine {
    stream: Option<cpal::Stream>,
    effects: Arc<SoundEffects>,
    active_ambient: Arc<Mutex<bool>>,
}

impl AudioEngine {
    /// Crear un nuevo motor de audio (sin inicializar stream aún)
    pub fn new(effects: Arc<SoundEffects>) -> Self {
        Self {
            stream: None,
            effects,
            active_ambient: Arc::new(Mutex::new(false)),
        }
    }

    /// Inicializar el stream de audio con el host por defecto
    pub fn init(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No se encontró dispositivo de audio de salida".to_string())?;

        let config = device
            .default_output_config()
            .map_err(|e| format!("Error al obtener configuración de audio: {}", e))?;

        let effects = Arc::clone(&self.effects);
        let active_ambient = Arc::clone(&self.active_ambient);
        let sample_rate = config.sample_rate().0;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                device
                    .build_output_stream(
                        &config.into(),
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            audio_callback_f32(data, &effects, &active_ambient, sample_rate);
                        },
                        |err| eprintln!("Error en stream de audio: {}", err),
                        None,
                    )
                    .map_err(|e| format!("Error al construir stream: {}", e))?
            }
            _ => {
                return Err("Formato de sample no soportado (solo F32)".to_string());
            }
        };

        stream
            .play()
            .map_err(|e| format!("Error al iniciar stream: {}", e))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Activar sonido ambiente
    pub fn set_ambient(&self, on: bool) {
        if let Ok(mut active) = self.active_ambient.lock() {
            *active = on;
        }
    }

    /// Reproducir un efecto de sonido one-shot
    pub fn play(&self, event: AudioEvent) {
        match event {
            AudioEvent::AmbientOn => self.set_ambient(true),
            AudioEvent::AmbientOff => self.set_ambient(false),
            _ => {
                let effects = Arc::clone(&self.effects);
                std::thread::spawn(move || {
                    play_one_shot(&effects, event);
                });
            }
        }
    }
}

/// Pre-generar todos los efectos de audio
pub fn generate_sound_effects(sample_rate: u32) -> SoundEffects {
    let click = generate_sine_wave(440.0, 0.05, sample_rate);
    let build_complete = generate_sweep(200.0, 800.0, 0.5, sample_rate);
    let car_horn = generate_square_wave(300.0, 0.2, sample_rate);
    let city_ambient = generate_noise(4.0, sample_rate);

    SoundEffects {
        click,
        build_complete,
        car_horn,
        city_ambient,
    }
}

// ─── Callback de audio ──────────────────────────────────────────────────────

fn audio_callback_f32(
    data: &mut [f32],
    effects: &SoundEffects,
    active_ambient: &Mutex<bool>,
    _sample_rate: u32,

) {
    let play_ambient = active_ambient.lock().map(|a| *a).unwrap_or(false);

    if play_ambient && !effects.city_ambient.samples.is_empty() {
        let ambient = &effects.city_ambient.samples;
        let len = ambient.len();
        for (i, sample) in data.iter_mut().enumerate() {
            *sample = ambient[i % len];
        }
    } else {
        for sample in data.iter_mut() {
            *sample = 0.0;
        }
    }
}

fn play_one_shot(effects: &SoundEffects, event: AudioEvent) {
    let buffer = match event {
        AudioEvent::Click => &effects.click,
        AudioEvent::BuildComplete => &effects.build_complete,
        AudioEvent::CarHorn => &effects.car_horn,
        _ => return,
    };

    if buffer.samples.is_empty() {
        return;
    }

    if let Ok(host) = (|| -> Result<cpal::Host, cpal::HostUnavailable> {
        Ok(cpal::default_host())
    })() {
        if let Some(device) = host.default_output_device() {
            if let Ok(config) = device.default_output_config() {
                let samples = buffer.samples.clone();
                let _ = device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        for (i, sample) in data.iter_mut().enumerate() {
                            *sample = if i < samples.len() { samples[i] } else { 0.0 };
                        }
                    },
                    |err| eprintln!("Error en one-shot: {}", err),
                    None,
                );
            }
        }
    }
}

// ─── Generadores de formas de onda ──────────────────────────────────────────
// [TC#5] LUTs trigonométricas precalculadas

fn generate_sine_wave(freq: f32, duration_secs: f32, sample_rate: u32) -> AudioBuffer {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let phase = 2.0 * std::f32::consts::PI * freq * t;
        let sample = luts::sin_fast(phase) * 0.3; // Volumen al 30%
        // Envelope ADSR simplificado
        let envelope = if i < num_samples / 10 {
            // Attack
            i as f32 / (num_samples / 10) as f32
        } else if i > num_samples * 9 / 10 {
            // Release
            (num_samples - i) as f32 / (num_samples / 10) as f32
        } else {
            1.0
        };
        samples.push(sample * envelope);
    }

    AudioBuffer {
        samples,
        sample_rate,
    }
}

fn generate_sweep(
    start_freq: f32,
    end_freq: f32,
    duration_secs: f32,
    sample_rate: u32,
) -> AudioBuffer {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration_secs;
        let freq = start_freq + (end_freq - start_freq) * progress;
        let phase = 2.0 * std::f32::consts::PI * freq * t;
        let sample = luts::sin_fast(phase) * 0.25;
        let envelope = 1.0 - progress; // Fade out
        samples.push(sample * envelope);
    }

    AudioBuffer {
        samples,
        sample_rate,
    }
}

fn generate_square_wave(freq: f32, duration_secs: f32, sample_rate: u32) -> AudioBuffer {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let phase = (freq * t).fract();
        let sample = if phase < 0.5 { 0.3 } else { -0.3 };
        samples.push(sample);
    }

    AudioBuffer {
        samples,
        sample_rate,
    }
}

fn generate_noise(duration_secs: f32, sample_rate: u32) -> AudioBuffer {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    // [TC#22] RNG pool: usamos rand::SmallRng pre-inicializado
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);

    for _ in 0..num_samples {
        samples.push((rng.gen::<f32>() * 2.0 - 1.0) * 0.05); // Ruido blanco bajo
    }

    AudioBuffer {
        samples,
        sample_rate,
    }
}

// ─── AudioPlayer (wrapper público para main.rs) ──────────────────────────────

// ─── AudioPlayer (wrapper público para main.rs) ──────────────────────────────

/// Wrapper público alrededor de AudioEngine.
/// Mantiene compatibilidad con el código existente que espera `AudioPlayer`.
pub struct AudioPlayer {
    engine: AudioEngine,
    effects: Arc<SoundEffects>,
}

impl AudioPlayer {
    pub fn init() -> Self {
        let effects = Arc::new(generate_sound_effects(44100));
        let mut engine = AudioEngine::new(Arc::clone(&effects));
        let _ = engine.init(); // El stream puede fallar sin dispositivo; no es fatal
        AudioPlayer { engine, effects }
    }

    pub fn play_ambient(&mut self) {
        self.engine.set_ambient(true);
        self.engine.play(AudioEvent::AmbientOn);
    }

    pub fn play(&mut self, event: AudioEvent) {
        self.engine.play(event);
    }
}
