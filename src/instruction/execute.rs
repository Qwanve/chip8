use crate::ExitReason;
use bitvec::prelude::*;
use core::cmp::min;
use core::time::Duration;
use log::*;
use std::ops::ControlFlow;
use ux::u12;
use ux::u4;

pub enum DecodedInstr {
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

impl crate::State {
    pub fn execute(&mut self, instr: DecodedInstr) -> ControlFlow<ExitReason> {
        //TODO: Break this function up?
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
                info!("Copying register {y} to register {x}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x = y;
            }
            OrRegisters { x, y } => {
                info!("Oring register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x |= y;
                self.registers[u4::new(0xF)] = 0;
            }
            AndRegisters { x, y } => {
                info!("Adding register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x &= y;
                self.registers[u4::new(0xF)] = 0;
            }
            XorRegisters { x, y } => {
                info!("Xoring register {x} with register {y}");
                let y = self.registers[y];
                let x = &mut self.registers[x];
                *x ^= y;
                self.registers[u4::new(0xF)] = 0;
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
                info!("Adding register {y} to register {x}");
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
                let bytes = u8::from(bytes);
                let x = x % 0x40;
                let y = y % 0x20;
                info!("Drawing sprite at {x},{y} with size {bytes}");
                std::thread::sleep(Duration::from_secs_f32(1f32 / 60f32));

                let mut vram = self.vram.lock().unwrap();
                let mut collision = false;
                for b in 0..bytes {
                    //Drawing past the bottom
                    if y + b >= 32 {
                        debug!("Drawing past the bottom of the frame");
                        break;
                    }
                    let byte = self.memory[self.vi + u16::from(b)];
                    debug!("Drawing line {b}, value: {byte:X}");
                    let bits = byte.view_bits::<Msb0>();
                    let start = usize::from(y + b) * 64 + usize::from(x);
                    let end = usize::from(y + b) * 64 + min(usize::from(x) + 8, 63);
                    {
                        let write_area = &mut vram[start..=end];
                        write_area.iter_mut().zip(bits).for_each(|(v, s)| {
                            if *v && *s {
                                collision = true;
                            }
                            *v ^= *s
                        });
                    }
                }
                self.registers[u4::new(0xF)] = collision as u8;
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

                if let Some(key) = self.last_key_press {
                    debug!("Got key press: {key}");
                    self.registers[register] = key;
                    self.last_key_press = None;
                } else {
                    debug!("Registering wait for key press");
                    return ControlFlow::Break(ExitReason::WaitingForKeyPress);
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
