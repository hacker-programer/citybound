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

// ---------------------------------------------------------------------------
// GENERACIÓN DE TONOS
// ---------------------------------------------------------------------------

impl AudioBuffer {
    pub fn silence(duration_secs: f32, sample_rate: u32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        AudioBuffer {
            samples: vec![0.0_f32; num_samples],
            sample_rate,
        }
    }

    pub fn sine_wave(frequency: f32, duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = luts::sin_fast(2.0 * std::f32::consts::PI * frequency * t) * volume;
            samples.push(sample);
        }

        // Fade out para evitar click
        let fade_samples = (0.01 * sample_rate as f32) as usize;
        if fade_samples > 0 && fade_samples < num_samples {
            for i in (num_samples - fade_samples)..num_samples {
                let fade = 1.0 - (i - (num_samples - fade_samples)) as f32 / fade_samples as f32;
                samples[i] *= fade;
            }
        }

        AudioBuffer { samples, sample_rate }
    }

    pub fn square_wave(frequency: f32, duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let period = sample_rate as f32 / frequency;

        for i in 0..num_samples {
            let phase = (i as f32 % period) / period;
            let sample = if phase < 0.5 { volume } else { -volume };
            samples.push(sample);
        }

        AudioBuffer { samples, sample_rate }
    }

    pub fn white_noise(duration_secs: f32, sample_rate: u32, volume: f32) -> Self {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut samples = Vec::with_capacity(num_samples);
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

        AudioBuffer { samples, sample_rate }
    }
}

// ---------------------------------------------------------------------------
// GENERADOR DE EFECTOS
// ---------------------------------------------------------------------------

impl SoundEffects {
    pub fn generate_all() -> Self {
        let sample_rate: u32 = 44100;

        SoundEffects {
            click: AudioBuffer::sine_wave(440.0, 0.05, sample_rate, 0.3),
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
            car_horn: AudioBuffer::square_wave(300.0, 0.2, sample_rate, 0.5),
            city_ambient: AudioBuffer::white_noise(2.0, sample_rate, 0.03),
        }
    }
}

// ---------------------------------------------------------------------------
// REPRODUCTOR DE AUDIO (con cpal)
// ---------------------------------------------------------------------------

/// Cola de eventos thread-safe
struct EventQueue {
    events: Vec<AudioEvent>,
    position: usize,
}

impl EventQueue {
    fn new() -> Self {
        EventQueue { events: Vec::with_capacity(64), position: 0 }
    }

    fn push(&mut self, event: AudioEvent) {
        self.events.push(event);
    }

    fn drain(&mut self) -> Vec<AudioEvent> {
        if self.position >= self.events.len() {
            self.events.clear();
            self.position = 0;
            return Vec::new();
        }
        let events: Vec<_> = self.events[self.position..].to_vec();
        self.events.clear();
        self.position = 0;
        events
    }
}

pub struct AudioPlayer {
    pub effects: SoundEffects,
    pub master_volume: f32,
    pub muted: bool,
    queue: Arc<Mutex<EventQueue>>,
    ambient_active: bool,
    ambient_offset: usize,
}

impl AudioPlayer {
    /// Inicializa el sistema de audio
    pub fn init() -> Self {
        let effects = SoundEffects::generate_all();
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let queue_clone = queue.clone();

        // Intentar iniciar thread de audio con cpal
        std::thread::spawn(move || {
            let host = cpal::default_host();
            if let Ok(Some(device)) = host.default_output_device() {
                if let Ok(config) = device.default_output_config() {
                    let _ = run_audio_thread(device, config, queue_clone);
                }
            }
        });

        AudioPlayer {
            effects,
            master_volume: 0.7,
            muted: false,
            queue,
            ambient_active: false,
            ambient_offset: 0,
        }
    }

    pub fn play_click(&mut self) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(AudioEvent::Click);
        }
    }

    pub fn play_build_complete(&mut self) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(AudioEvent::BuildComplete);
        }
    }

    pub fn play_car_horn(&mut self) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(AudioEvent::CarHorn);
        }
    }

    pub fn play_ambient(&mut self) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(AudioEvent::AmbientOn);
            self.ambient_active = true;
        }
    }

    pub fn stop_ambient(&mut self) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(AudioEvent::AmbientOff);
            self.ambient_active = false;
        }
    }

    /// Procesa eventos pendientes (llamar cada frame)
    pub fn tick(&mut self) {
        // La mayoría del trabajo ocurre en el thread de audio
        // Aquí solo gestionamos el estado
    }

    /// Devuelve el siguiente chunk de audio ambiente para loop
    pub fn get_ambient_chunk(&mut self, chunk_size: usize) -> Vec<f32> {
        let ambient = &self.effects.city_ambient.samples;
        if ambient.is_empty() {
            return vec![0.0; chunk_size];
        }
        let len = ambient.len();
        let mut chunk = Vec::with_capacity(chunk_size);
        for i in 0..chunk_size {
            let idx = (self.ambient_offset + i) % len;
            chunk.push(ambient[idx]);
        }
        self.ambient_offset = (self.ambient_offset + chunk_size) % len;
        chunk
    }
}

/// Thread de audio: envía samples al dispositivo cpal
fn run_audio_thread(
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    queue: Arc<Mutex<EventQueue>>,
) -> Result<(), String> {
    let sample_rate = config.sample_rate().0;

    // Generar efectos al sample rate del dispositivo
    let effects = SoundEffects::generate_all();

    let mut active_sounds: Vec<(Vec<f32>, usize)> = Vec::new(); // (buffer, position)
    let mut ambient_on = false;
    let mut ambient_pos: usize = 0;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // Procesar eventos de la cola
            if let Ok(mut q) = queue.lock() {
                for event in q.drain() {
                    match event {
                        AudioEvent::Click => {
                            active_sounds.push((effects.click.samples.clone(), 0));
                        }
                        AudioEvent::BuildComplete => {
                            active_sounds.push((effects.build_complete.samples.clone(), 0));
                        }
                        AudioEvent::CarHorn => {
                            active_sounds.push((effects.car_horn.samples.clone(), 0));
                        }
                        AudioEvent::AmbientOn => { ambient_on = true; }
                        AudioEvent::AmbientOff => { ambient_on = false; }
                    }
                }
            }

            // Generar samples
            for sample in data.iter_mut() {
                let mut sum: f32 = 0.0;

                // Mezclar sonidos activos
                let mut i = 0;
                while i < active_sounds.len() {
                    let (buffer, pos) = &mut active_sounds[i];
                    if *pos < buffer.len() {
                        sum += buffer[*pos] * 0.5;
                        *pos += 1;
                        i += 1;
                    } else {
                        active_sounds.swap_remove(i);
                    }
                }

                // Ambiente
                if ambient_on {
                    let amb = &effects.city_ambient.samples;
                    if !amb.is_empty() {
                        sum += amb[ambient_pos % amb.len()] * 0.3;
                        ambient_pos += 1;
                    }
                }

                *sample = sum.clamp(-1.0, 1.0);
            }
        },
        |err| { eprintln!("Error audio: {}", err); },
        None,
    ).map_err(|e| format!("Error al crear stream: {}", e))?;

    stream.play().map_err(|e| format!("Error al iniciar stream: {}", e))?;

    // Mantener vivo (el stream se dropea al salir de esta fn)
    std::thread::park();

    Ok(())
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
        assert_eq!(buf.samples.len(), 4410);
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
        let has_pos = buf.samples.iter().any(|&s| s > 0.0);
        let has_neg = buf.samples.iter().any(|&s| s < 0.0);
        assert!(has_pos && has_neg, "Square wave debe tener valores ±");
    }

    #[test]
    fn test_white_noise() {
        let buf = AudioBuffer::white_noise(0.5, 44100, 0.3);
        assert_eq!(buf.samples.len(), 22050);
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
        let mut player = AudioPlayer::init();
        assert_eq!(player.master_volume, 0.7);
        assert!(!player.muted);
        // No debería fallar
        player.play_click();
        player.tick();
    }

    #[test]
    fn test_silence() {
        let buf = AudioBuffer::silence(0.1, 44100);
        assert_eq!(buf.samples.len(), 4410);
        assert!(buf.samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_get_ambient_chunk() {
        crate::luts::init_trig_luts();
        let mut player = AudioPlayer::init();
        let chunk = player.get_ambient_chunk(100);
        assert_eq!(chunk.len(), 100);
    }
}
