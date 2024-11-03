use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

use core::time::Duration;
use log::*;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use smol::Timer;
use std::sync::Arc;
use std::sync::Mutex;

pub async fn sdl2(
    vram: Arc<Mutex<[bool; 64 * 32]>>,
    keypad: Arc<Mutex<Keypad>>,
    sound_timer: Arc<Mutex<u8>>,
) {
    info!("Warming up sdl system");
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let audio_subsystem = sdl_context.audio().unwrap();

    let desired_audio_spec = AudioSpecDesired {
        freq: None,
        channels: Some(1),
        samples: None,
    };

    let audio_device = audio_subsystem
        .open_playback(None, &desired_audio_spec, |spec| {
            // initialize the audio callback
            SquareWave {
                phase_inc: 880.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.25,
            }
        })
        .unwrap();

    let window = video_subsystem
        .window("chip8", 640, 320)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_logical_size(64, 32).unwrap();
    canvas.clear();

    let texcreator = canvas.texture_creator();
    let mut tex = texcreator
        .create_texture_streaming(PixelFormatEnum::RGB332, 64, 32)
        .unwrap();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    loop {
        let start = std::time::Instant::now();
        canvas.clear();
        for event in event_pump.poll_iter() {
            use Keycode::*;
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape | Keycode::Q),
                    ..
                } => {
                    info!("Recieved quit. Shutting down");
                    return;
                }
                #[rustfmt::skip]
                Event::KeyDown {
                    keycode:
                        keycode @ Some(
                            | Num4 | Num5 | Num6 | Num7
                            | R    | T    | Y    | U
                            | F    | G    | H    | J
                            | V    | B    | N    | M
                        ),
                    repeat: false,
                    ..
                } => {
                    let keycode = keycode.unwrap();
                    info!("Recieved keydown: {keycode}");
                    keypad.lock().unwrap().press(keycode);

                }
                #[rustfmt::skip]
                Event::KeyUp {
                    keycode:
                        keycode @ Some(
                            | Num4 | Num5 | Num6 | Num7
                            | R    | T    | Y    | U
                            | F    | G    | H    | J
                            | V    | B    | N    | M
                        ),
                    repeat: false,
                    ..
                } => {
                    let keycode = keycode.unwrap();
                    info!("Recieved keyup: {keycode}");
                    keypad.lock().unwrap().release(keycode);

                }
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        let beep = *sound_timer.lock().unwrap() > 1;
        if beep {
            audio_device.resume();
        } else {
            audio_device.pause();
        }

        let vram = vram.lock().unwrap().map(|pix| pix as u8 * 255);
        tex.update(None, &vram, 64).unwrap();

        trace!("Drawing frame");
        canvas.copy(&tex, None, None).unwrap();

        canvas.present();
        Timer::after(Duration::from_secs_f64(1f64 / 60f64).saturating_sub(start.elapsed())).await;
        let diff = start.elapsed().as_micros() as f64;
        trace!("FPS: {:.1}", 1f64 / (diff / 1000000.0));
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Keypad(pub [bool; 16]);

impl Keypad {
    pub fn press(&mut self, keycode: Keycode) {
        use Keycode::*;
        match keycode {
            Num4 => self.0[0x1] = true,
            Num5 => self.0[0x2] = true,
            Num6 => self.0[0x3] = true,
            Num7 => self.0[0xC] = true,
            R => self.0[0x4] = true,
            T => self.0[0x5] = true,
            Y => self.0[0x6] = true,
            U => self.0[0xD] = true,
            F => self.0[0x7] = true,
            G => self.0[0x8] = true,
            H => self.0[0x9] = true,
            J => self.0[0xE] = true,
            V => self.0[0xA] = true,
            B => self.0[0x0] = true,
            N => self.0[0xB] = true,
            M => self.0[0xF] = true,
            _ => {}
        };
    }
    pub fn release(&mut self, keycode: Keycode) {
        use Keycode::*;
        match keycode {
            Num4 => self.0[0x1] = false,
            Num5 => self.0[0x2] = false,
            Num6 => self.0[0x3] = false,
            Num7 => self.0[0xC] = false,
            R => self.0[0x4] = false,
            T => self.0[0x5] = false,
            Y => self.0[0x6] = false,
            U => self.0[0xD] = false,
            F => self.0[0x7] = false,
            G => self.0[0x8] = false,
            H => self.0[0x9] = false,
            J => self.0[0xE] = false,
            V => self.0[0xA] = false,
            B => self.0[0x0] = false,
            N => self.0[0xB] = false,
            M => self.0[0xF] = false,
            _ => {}
        };
    }

    pub fn is_pressed(&self, key: u8) -> bool {
        self.0[key as usize]
    }
    pub fn first_pressed(&self) -> Option<u8> {
        self.0.iter().position(|x| *x).map(|x| x as u8)
    }
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}
