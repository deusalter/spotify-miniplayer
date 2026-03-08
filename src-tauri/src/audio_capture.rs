use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;

const FFT_SIZE: usize = 2048;
const NUM_BARS: usize = 32;

#[derive(Clone, serde::Serialize)]
pub struct SpectrumData {
    pub magnitudes: Vec<f32>,
}

pub fn start_capture(on_spectrum: Channel<SpectrumData>) -> Result<(), String> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .ok_or("No input device available")?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {}", e))?;

    let sample_rate = config.sample_rate().0;
    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE * 2)));
    let buf_clone = buffer.clone();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buf = buf_clone.lock().unwrap();
                buf.extend_from_slice(data);

                while buf.len() >= FFT_SIZE {
                    let chunk: Vec<f32> = buf.drain(..FFT_SIZE).collect();
                    let hann = hann_window(&chunk);

                    if let Ok(spectrum) = samples_fft_to_spectrum(
                        &hann,
                        sample_rate,
                        FrequencyLimit::Range(20.0, 16000.0),
                        None,
                    ) {
                        let all_mags: Vec<f32> =
                            spectrum.data().iter().map(|(_, v)| v.val()).collect();

                        let bars = bucket_to_bars(&all_mags, NUM_BARS);
                        let _ = on_spectrum.send(SpectrumData { magnitudes: bars });
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {}", e))?;

    // Keep stream alive by leaking it (Stream is !Send on macOS so we can't move it to a thread)
    Box::leak(Box::new(stream));

    Ok(())
}

fn bucket_to_bars(magnitudes: &[f32], num_bars: usize) -> Vec<f32> {
    let len = magnitudes.len();
    if len == 0 {
        return vec![0.0; num_bars];
    }

    let mut bars = Vec::with_capacity(num_bars);
    for i in 0..num_bars {
        let start = ((i as f64 / num_bars as f64).powf(2.0) * len as f64) as usize;
        let end = (((i + 1) as f64 / num_bars as f64).powf(2.0) * len as f64) as usize;
        let start = start.min(len);
        let end = end.max(start + 1).min(len);

        let avg: f32 = magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;
        bars.push(avg);
    }

    let max = bars.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for b in &mut bars {
            *b /= max;
        }
    }

    bars
}
