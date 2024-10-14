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
    let keypad = Mutex::new(Keypad([false; 16]));
    let delay_timer = Mutex::new(0);
    let file = std::env::args()
        .nth(1)
        .expect("Expected rom as first arguement");
    info!("Opening rom");
    let rom = std::fs::read(file).unwrap();
    let mut state = State::new(&vram, &keypad, &delay_timer, rom);
    let mut disp = pin!(sdl2(&vram, &keypad).fuse());
    smol::block_on(async {
        select! {
            _ = disp => return,
            _ = handle_delay_timer(&delay_timer).fuse() => {},
            reason = state.run().fuse() => error!("Core returned: {reason:?}"),
        };
        disp.await;
    });
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct Keypad([bool; 16]);

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
struct Registers([u8; 16]);

impl Index<u4> for Registers {
    type Output = u8;
    fn index(&self, idx: u4) -> &Self::Output {
        trace!("Accessing register {idx:#X}");
        &self.0[usize::from(u8::from(idx))]
    }
}

impl IndexMut<u4> for Registers {
    fn index_mut(&mut self, idx: u4) -> &mut Self::Output {
        trace!("Accessing register {idx:#X}");
        &mut self.0[usize::from(u8::from(idx))]
    }
}

#[derive(Clone)]
struct State<'vram, 'keypad, 'dt> {
    pc: u16,
    vram: &'vram Mutex<[bool; 64 * 32]>,
    memory: Memory,
    stack: Vec<u16>,
    registers: Registers,
    vi: u16,
    keypad: &'keypad Mutex<Keypad>,
    delay_timer: &'dt Mutex<u8>,
}
impl State<'_, '_, '_> {
    fn new<'vram, 'keypad, 'dt>(
        vram: &'vram Mutex<[bool; 64 * 32]>,
        keypad: &'keypad Mutex<Keypad>,
        delay_timer: &'dt Mutex<u8>,
        rom: Vec<u8>,
    ) -> State<'vram, 'keypad, 'dt> {
        State {
            pc: 0x200,
            vram,
            memory: Memory { rom },
            stack: Vec::new(),
            registers: Registers([0; 16]),
            vi: 0,
            keypad,
            delay_timer,
        }
    }

    async fn run(&mut self) -> ControlFlow<ExitReason> {
        loop {
            let instr = self.fetch();
            debug!("{:04X}: {instr:04X?}", self.pc);
            let instr = instr.decode();
            self.execute(instr)?;
            // futures::pending!();
            Timer::after(Duration::from_secs_f32(1f32 / 100f32)).await;
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
                info!("Clearing Screen");
                let mut vram = self.vram.lock().unwrap();
                *vram = [false; 64 * 32];
            }
            Return => {
                info!("Return");
                if let Some(addr) = self.stack.pop() {
                    self.pc = addr;
                } else {
                    warn!("Return without address on stack");
                }
            }
            Jump { address } => {
                info!("Jumping to {address:03X}");
                if self.pc - 2 == address.into() {
                    return ControlFlow::Break(ExitReason::InfiniteLoop);
                }
                self.pc = address.into();
            }
            Call { address } => {
                info!("Call to address {address:03X}");
                self.stack.push(self.pc);
                self.pc = u16::from(address);
            }
            SkipIfEqual { register, value } => {
                info!("Skipping if register {register} is {value:X}");
                let reg = self.registers[register];
                if reg == value {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            SkipIfNotEqual { register, value } => {
                info!("Skipping if register {register} is not {value:X}");
                let reg = self.registers[register];
                if reg != value {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            SkipIfRegisterEqual { x, y } => {
                info!("Skipping if register {x} is equal to register {y}");
                let x = self.registers[x];
                let y = self.registers[y];
                if x == y {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            LoadRegister { register, value } => {
                info!("Load register {register} with {value:02X}");
                self.registers[register] = value;
            }
            AddToRegister { register, value } => {
                info!("Adding {value:02X} to register {register}");
                let reg = &mut self.registers[register];
                *reg = reg.wrapping_add(value);
            }
            CopyRegister { x, y } => {
                info!("Copying register {x} to register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x = y;
            }
            OrRegisters { x, y } => {
                info!("Oring register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x |= y;
            }
            AndRegisters { x, y } => {
                info!("Adding register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x &= y;
            }
            XorRegisters { x, y } => {
                info!("Xoring register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x ^= y;
            }
            SkipIfRegisterNotEqual { x, y } => {
                info!("Skipping if register {x} is not equal to register {y}");
                let x = self.registers[x];
                let y = self.registers[y];
                if x != y {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            AddRegisters { x, y } => {
                info!("Adding register {x} to register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                let (result, carry) = x.overflowing_add(y);
                *x = result;
                let flags = &mut self.registers[u4::new(0xF)];
                *flags = u8::from(carry);
            }
            SubtractRegisters { x, y } => {
                info!("Subtracting register {y} from register {x}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                let (result, carry) = x.overflowing_sub(y);
                *x = result;
                let flags = &mut self.registers[u4::new(0xF)];
                *flags = u8::from(!carry);
            }
            ShiftRight { x, y } => {
                info!("Setting register {x} to shifted register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                let lsb = y & 0b1;
                *x = y >> 1;
                let flags = &mut self.registers[u4::new(0xF)];
                *flags = lsb;
            }
            SubtractRegistersReverse { x, y } => {
                info!("Subtracting register {y} from register {x}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                let (result, carry) = y.overflowing_sub(*x);
                *x = result;
                let flags = &mut self.registers[u4::new(0xF)];
                *flags = u8::from(!carry);
            }
            ShiftLeft { x, y } => {
                info!("Setting register {x} to shifted register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                let msb = (y & 0b1000_0000) >> 7;
                *x = y << 1;
                let flags = &mut self.registers[u4::new(0xF)];
                *flags = msb;
            }
            LoadIRegister { value } => {
                info!("Load register I with {value:02X}");
                self.vi = value.into();
            }
            JumpWithOffset { address } => {
                info!("Jumping to address {address:04X} + V0");
                let reg = self.registers[u4::new(0)];
                self.pc = u16::from(address).wrapping_add(u16::from(reg));
            }
            DrawSprite { x, y, bytes } => {
                let x = self.registers[x];
                let y = self.registers[y];
                info!("Drawing sprint at {x},{y} with size {bytes}");
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
            SkipIfPressed { key } => {
                info!("Skipping if key in register {key} is pressed");
                let key = self.registers[key];
                debug!("Key: {key}");
                let pressed = self.keypad.lock().unwrap().is_pressed(key);
                if pressed {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            SkipIfNotPressed { key } => {
                info!("Skipping if key in register {key} is not pressed");
                let key = self.registers[key];
                debug!("Key: {key}");
                let pressed = self.keypad.lock().unwrap().is_pressed(key);
                if !pressed {
                    trace!("Skipped");
                    self.pc += 2;
                }
            }
            StoreDelayTimer { register } => {
                info!("Storing delay timer in register {register}");
                self.registers[register] = *self.delay_timer.lock().unwrap();
            }
            WaitForKeyPress { register } => {
                info!("Waiting for keypress to put in register {register}");
                if let Some(key) = self.keypad.lock().unwrap().first_pressed() {
                    debug!("Key pressed: {key}");
                    self.registers[register] = key;
                } else {
                    debug!("Still waiting");
                    self.pc -= 2;
                }
            }
            SetDelayTimer { register } => {
                info!("Setting delay timer to register {register}");
                *self.delay_timer.lock().unwrap() = self.registers[register];
            }
            AddToIRegister { register } => {
                info!("Adding register {register} to I");
                self.vi += u16::from(self.registers[register]);
            }
            BinaryCodedDecimal { register } => {
                info!("Converting register {register} to decimal");
                //TODO: Better algorithm
                let value = self.registers[register];
                let decimal = format!("{value:03}");
                for (idx, digit) in decimal.chars().take(3).enumerate() {
                    let idx = u16::try_from(idx).unwrap();
                    let digit = u8::try_from(digit.to_digit(10).unwrap()).unwrap();
                    self.memory[self.vi + idx] = digit;
                }
                // self.vi += 3;
            }
            StoreRegisters { register } => {
                info!("Storing registers 0 - {register}");
                for x in 0..=u8::from(register) {
                    self.memory[self.vi + u16::from(x)] = self.registers[u4::new(x)];
                }
                self.vi += u16::from(register) + 1;
            }
            LoadRegisters { register } => {
                info!("Loading registers 0 - {register}");
                for x in 0..=u8::from(register) {
                    self.registers[u4::new(x)] = self.memory[self.vi + u16::from(x)];
                }
                self.vi += u16::from(register) + 1;
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
    Return,
    Jump { address: u12 },
    Call { address: u12 },
    SkipIfEqual { register: u4, value: u8 },
    SkipIfNotEqual { register: u4, value: u8 },
    SkipIfRegisterEqual { x: u4, y: u4 },
    LoadRegister { register: u4, value: u8 },
    CopyRegister { x: u4, y: u4 },
    OrRegisters { x: u4, y: u4 },
    AndRegisters { x: u4, y: u4 },
    XorRegisters { x: u4, y: u4 },
    AddToRegister { register: u4, value: u8 },
    SkipIfRegisterNotEqual { x: u4, y: u4 },
    AddRegisters { x: u4, y: u4 },
    SubtractRegisters { x: u4, y: u4 },
    ShiftRight { x: u4, y: u4 },
    SubtractRegistersReverse { x: u4, y: u4 },
    ShiftLeft { x: u4, y: u4 },
    LoadIRegister { value: u12 },
    JumpWithOffset { address: u12 },
    DrawSprite { x: u4, y: u4, bytes: u4 },
    SkipIfPressed { key: u4 },
    SkipIfNotPressed { key: u4 },
    StoreDelayTimer { register: u4 },
    WaitForKeyPress { register: u4 },
    SetDelayTimer { register: u4 },
    AddToIRegister { register: u4 },
    BinaryCodedDecimal { register: u4 },
    StoreRegisters { register: u4 },
    LoadRegisters { register: u4 },
    IllegalInstruction(u16),
}

impl Instr {
    fn decode(self) -> DecodedInstr {
        match self.0 {
            0x00E0 => DecodedInstr::ClearScreen,
            0x00EE => DecodedInstr::Return,
            0x1000..=0x1FFF => DecodedInstr::Jump {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0x2000..=0x2FFF => DecodedInstr::Call {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0x3000..=0x3FFF => DecodedInstr::SkipIfEqual {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x4000..=0x4FFF => DecodedInstr::SkipIfNotEqual {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x5000..=0x5FFF if self.0 & 0xF == 0 => DecodedInstr::SkipIfRegisterEqual {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
            },
            0x6000..=0x6FFF => DecodedInstr::LoadRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x7000..=0x7FFF => DecodedInstr::AddToRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x8000..=0x8FFF => match self.0 & 0xF {
                0x0 => DecodedInstr::CopyRegister {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x1 => DecodedInstr::OrRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x2 => DecodedInstr::AndRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x3 => DecodedInstr::XorRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x4 => DecodedInstr::AddRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x5 => DecodedInstr::SubtractRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x6 => DecodedInstr::ShiftRight {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x7 => DecodedInstr::SubtractRegistersReverse {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0xE => DecodedInstr::ShiftLeft {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x8..=0xD | 0xF => DecodedInstr::IllegalInstruction(self.0),
                0x10.. => unreachable!(),
            },
            0x9000..=0x9FFF if self.0 & 0xF == 0 => DecodedInstr::SkipIfRegisterNotEqual {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
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
            0xE000..=0xEFFF => match u8::try_from(self.0 & 0xFF).unwrap() {
                0x9E => DecodedInstr::SkipIfPressed {
                    key: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0xA1 => DecodedInstr::SkipIfNotPressed {
                    key: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                _ => DecodedInstr::IllegalInstruction(self.0),
            },
            0xF000..=0xFFFF => match u8::try_from(self.0 & 0xFF).unwrap() {
                0x07 => DecodedInstr::StoreDelayTimer {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x0A => DecodedInstr::WaitForKeyPress {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x15 => DecodedInstr::SetDelayTimer {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x1E => DecodedInstr::AddToIRegister {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x33 => DecodedInstr::BinaryCodedDecimal {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x55 => DecodedInstr::StoreRegisters {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x65 => DecodedInstr::LoadRegisters {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                _ => DecodedInstr::IllegalInstruction(self.0),
            },
            _ => DecodedInstr::IllegalInstruction(self.0),
        }
    }
}

async fn handle_delay_timer(delay_timer: &Mutex<u8>) {
    loop {
        let mut delay_timer = delay_timer.lock().unwrap();
        *delay_timer = delay_timer.saturating_sub(1);
        drop(delay_timer);
        Timer::after(Duration::from_secs_f64(1f64 / 60f64)).await;
    }
}

async fn sdl2(vram: &Mutex<[bool; 64 * 32]>, keypad: &Mutex<Keypad>) {
    info!("Warming up sdl system");
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

        trace!("Drawing frame");
        tex.with_lock(None, |buf, _| {
            let vram = vram.lock().unwrap();
            vram.iter()
                .map(|pix| if *pix { 255 } else { 0 })
                .zip(buf)
                .for_each(|(new, old)| *old = new);
        })
        .unwrap();
        canvas.copy(&tex, None, None).unwrap();

        canvas.present();
        let end = std::time::Instant::now();
        Timer::after(Duration::from_secs_f64(1f64 / 60f64) - (end - start)).await;
        let end = std::time::Instant::now();
        let diff = (end - start).as_micros() as f64;
        trace!("FPS: {:.1}", 1f64 / (diff / 1000000.0));
    }
}

impl Keypad {
    fn press(&mut self, keycode: Keycode) -> &mut Self {
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
        self
    }
    fn release(&mut self, keycode: Keycode) -> &mut Self {
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
        self
    }

    fn is_pressed(&self, key: u8) -> bool {
        self.0[key as usize]
    }
    fn first_pressed(&self) -> Option<u8> {
        self.0.iter().position(|x| *x).map(|x| x as u8)
    }
}
