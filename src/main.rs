use bitvec::prelude::*;
use core::cmp::min;
use core::ops::Index;
use core::ops::IndexMut;
use core::pin::pin;
use core::time::Duration;
use futures::select;
use futures::FutureExt;
use log::*;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use smol::Timer;
use std::ops::ControlFlow;
use std::sync::Mutex;
use ux::u12;
use ux::u4;

fn main() {
    env_logger::init();
    let vram = Mutex::<[bool; 64 * 32]>::new([false; 64 * 32]);
    let file = std::env::args()
        .nth(1)
        .expect("Expected rom as first arguement");
    debug!("Opening rom");
    let rom = std::fs::read(file).unwrap();
    let mut state = State::new(&vram, rom);
    let mut disp = pin!(sdl2(&vram).fuse());
    smol::block_on(async {
        select! {
            _ = disp => return,
            reason = state.run().fuse() => error!("Core returned: {reason:?}"),
        };
        disp.await;
    });
}

#[derive(Clone)]
struct Memory {
    rom: Vec<u8>,
}

impl Index<u16> for Memory {
    type Output = u8;
    fn index(&self, idx: u16) -> &Self::Output {
        trace!("Accessing memory {idx:#X}");
        match idx {
            0x200.. => &self.rom[usize::from(idx) - 0x200],
            _ => todo!(),
        }
    }
}
impl IndexMut<u16> for Memory {
    fn index_mut(&mut self, idx: u16) -> &mut Self::Output {
        trace!("Accessing memory {idx:#X}");
        match idx {
            0x200.. => &mut self.rom[usize::from(idx) - 0x200],
            _ => todo!(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ExitReason {
    InfiniteLoop,
    IllegalInstruction,
}

#[derive(Clone)]
struct State<'vram> {
    pc: u16,
    vram: &'vram Mutex<[bool; 64 * 32]>,
    memory: Memory,
    registers: [u8; 16],
    vi: u16,
}
impl State<'_> {
    fn new<'vram>(vram: &'vram Mutex<[bool; 64 * 32]>, rom: Vec<u8>) -> State<'vram> {
        State {
            pc: 0x200,
            vram,
            memory: Memory { rom },
            registers: [0; 16],
            vi: 0,
        }
    }

    async fn run(&mut self) -> ControlFlow<ExitReason> {
        loop {
            let instr = self.fetch();
            info!("{:04X}: {instr:04X?}", self.pc);
            let instr = instr.decode();
            futures::pending!();
            // Timer::after(Duration::from_millis(30)).await;
            self.execute(instr)?;
        }
    }

    fn fetch(&self) -> Instr {
        Instr(u16::from_be_bytes([
            self.memory[self.pc],
            self.memory[self.pc + 1],
        ]))
    }

    fn execute(&mut self, instr: DecodedInstr) -> ControlFlow<ExitReason> {
        self.pc += 2;
        use DecodedInstr::*;
        match instr {
            ClearScreen => {
                debug!("ClearScreen");
                let mut vram = self.vram.lock().unwrap();
                *vram = [false; 64 * 32];
            }
            Jump { address } => {
                debug!("Jumping to {address:X}");
                if self.pc - 2 == address.into() {
                    return ControlFlow::Break(ExitReason::InfiniteLoop);
                }
                self.pc = address.into();
            }
            SkipIfEqual { register, value } => {
                debug!("Skipping if register {register} is {value:X}");
                let reg = self.registers[usize::from(u16::from(register))];
                if reg == value {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            LoadRegister { register, value } => {
                debug!("Load register {register} with {value:02X}");
                self.registers[usize::from(u16::from(register))] = value;
            }
            AddToRegister { register, value } => {
                debug!("Adding {value:02X} to register {register}");
                let reg = &mut self.registers[usize::from(u16::from(register))];
                *reg = reg.wrapping_add(value);
            }
            LoadIRegister { value } => {
                debug!("Load register I with {value:02X}");
                self.vi = value.into();
            }
            JumpWithOffset { address } => {
                debug!("Jumping to address {address:04X} + V0");
                let reg = self.registers[0];
                self.pc = u16::from(address).wrapping_add(u16::from(reg));
            }
            DrawSprite { x, y, bytes } => {
                let x = self.registers[usize::from(u16::from(x))];
                let y = self.registers[usize::from(u16::from(y))];
                debug!("Drawing sprint at {x},{y} with size {bytes}");
                let bytes = u8::from(bytes);
                let x = x % 0x3F;
                let y = y % 0x1F;

                for b in 0..bytes {
                    if y + b > 32 {
                        break;
                    }
                    let byte = self.memory[self.vi + u16::from(b)];
                    debug!("Drawing line {b}, value: {byte:X}");
                    let bits = byte.view_bits::<Msb0>();
                    let start = usize::from(y + b) * 64 + usize::from(x);
                    let end = usize::from(y + b) * 64 + usize::from(min(x + 8, 63));
                    {
                        let mut vram = self.vram.lock().unwrap();
                        let write_area = &mut vram[start..=end];
                        write_area.into_iter().zip(bits).for_each(|(v, s)| *v ^= *s);
                    }
                }
            }
            DecodedInstr::IllegalInstruction(instr) => {
                error!("Recieved illegal instruction: {instr:04X}");
                return ControlFlow::Break(ExitReason::IllegalInstruction);
            }
        };
        ControlFlow::Continue(())
    }
}

#[derive(Debug)]
struct Instr(u16);

enum DecodedInstr {
    ClearScreen,
    Jump { address: u12 },
    SkipIfEqual { register: u4, value: u8 },
    LoadRegister { register: u4, value: u8 },
    AddToRegister { register: u4, value: u8 },
    LoadIRegister { value: u12 },
    JumpWithOffset { address: u12 },
    DrawSprite { x: u4, y: u4, bytes: u4 },
    IllegalInstruction(u16),
}

impl Instr {
    fn decode(self) -> DecodedInstr {
        match self.0 {
            0x00E0 => DecodedInstr::ClearScreen,
            0x1000..=0x1FFF => DecodedInstr::Jump {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0x3000..=0x3FFF => DecodedInstr::SkipIfEqual {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x6000..=0x6FFF => DecodedInstr::LoadRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x7000..=0x7FFF => DecodedInstr::AddToRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0xA000..=0xAFFF => DecodedInstr::LoadIRegister {
                value: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0xB000..=0xBFFF => DecodedInstr::JumpWithOffset {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0xD000..=0xDFFF => DecodedInstr::DrawSprite {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                bytes: (self.0 & 0x000F).try_into().unwrap(),
            },
            _ => DecodedInstr::IllegalInstruction(self.0),
        }
    }
}

async fn sdl2(vram: &Mutex<[bool; 64 * 32]>) {
    debug!("Warming up sdl system");
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl2 demo", 640, 320)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_logical_size(64, 32).unwrap();
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    loop {
        canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape | Keycode::Q),
                    ..
                } => {
                    info!("Recieved quit. Shutting down");
                    return;
                }
                _ => {}
            }
        }
        // The rest of the game loop goes here...
        {
            trace!("Drawing frame");
            let texcreator = canvas.texture_creator();
            let mut tex = texcreator
                .create_texture_streaming(PixelFormatEnum::ABGR8888, 64, 32)
                .unwrap();
            tex.with_lock(None, |buf, _| {
                let vram = vram.lock().unwrap();
                vram.iter()
                    .map(|pix| {
                        if *pix {
                            [255, 255, 255, 255]
                        } else {
                            [0, 0, 0, 255]
                        }
                    })
                    .flatten()
                    .zip(buf)
                    .for_each(|(new, old)| *old = new);
            })
            .unwrap();
            canvas.copy(&tex, None, None).unwrap();
        }

        canvas.present();
        Timer::after(Duration::new(0, 1_000_000_000u32 / 60)).await;
    }
}
