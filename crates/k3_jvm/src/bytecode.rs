#[derive(Debug, Clone, Default)]
pub struct CodeBuilder {
    pub bytes: Vec<u8>,
    pub max_stack: u16,
    pub max_locals: u16,
    current_stack: u16,
}

impl CodeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stack_depth(&self) -> u16 {
        self.current_stack
    }

    pub fn set_stack_depth(&mut self, depth: u16) {
        self.current_stack = depth;
        if self.current_stack > self.max_stack {
            self.max_stack = self.current_stack;
        }
    }

    fn push_stack(&mut self, n: u16) {
        self.current_stack += n;
        if self.current_stack > self.max_stack {
            self.max_stack = self.current_stack;
        }
    }

    fn pop_stack(&mut self, n: u16) {
        self.current_stack = self.current_stack.saturating_sub(n);
    }

    pub fn aconst_null(&mut self) {
        self.bytes.push(0x01);
        self.push_stack(1);
    }

    pub fn iconst(&mut self, val: i32) {
        match val {
            -1 => self.bytes.push(0x02),
            0 => self.bytes.push(0x03),
            1 => self.bytes.push(0x04),
            2 => self.bytes.push(0x05),
            3 => self.bytes.push(0x06),
            4 => self.bytes.push(0x07),
            5 => self.bytes.push(0x08),
            -128..=127 => {
                self.bytes.push(0x10); // bipush
                self.bytes.push(val as i8 as u8);
            }
            -32768..=32767 => {
                self.bytes.push(0x11); // sipush
                self.bytes.extend_from_slice(&(val as i16).to_be_bytes());
            }
            _ => {
                // For larger ints, handled via ldc
                self.bytes.push(0x10);
                self.bytes.push(0);
            }
        }
        self.push_stack(1);
    }

    pub fn ldc(&mut self, cp_index: u16) {
        if cp_index <= 255 {
            self.bytes.push(0x12); // ldc
            self.bytes.push(cp_index as u8);
        } else {
            self.bytes.push(0x13); // ldc_w
            self.bytes.extend_from_slice(&cp_index.to_be_bytes());
        }
        self.push_stack(1);
    }

    pub fn ldc2_w(&mut self, cp_index: u16) {
        self.bytes.push(0x14); // ldc2_w
        self.bytes.extend_from_slice(&cp_index.to_be_bytes());
        self.push_stack(2);
    }

    pub fn lconst_0(&mut self) {
        self.bytes.push(0x09);
        self.push_stack(2);
    }

    pub fn lconst_1(&mut self) {
        self.bytes.push(0x0a);
        self.push_stack(2);
    }

    pub fn dconst_0(&mut self) {
        self.bytes.push(0x0e);
        self.push_stack(2);
    }

    pub fn dconst_1(&mut self) {
        self.bytes.push(0x0f);
        self.push_stack(2);
    }

    pub fn iload(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x1a),
            1 => self.bytes.push(0x1b),
            2 => self.bytes.push(0x1c),
            3 => self.bytes.push(0x1d),
            _ => {
                self.bytes.push(0x15);
                self.bytes.push(slot as u8);
            }
        }
        self.push_stack(1);
    }

    pub fn aload(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x2a),
            1 => self.bytes.push(0x2b),
            2 => self.bytes.push(0x2c),
            3 => self.bytes.push(0x2d),
            _ => {
                self.bytes.push(0x19);
                self.bytes.push(slot as u8);
            }
        }
        self.push_stack(1);
    }

    pub fn lload(&mut self, slot: u16) {
        if slot + 1 >= self.max_locals {
            self.max_locals = slot + 2;
        }
        match slot {
            0 => self.bytes.push(0x1e),
            1 => self.bytes.push(0x1f),
            2 => self.bytes.push(0x20),
            3 => self.bytes.push(0x21),
            _ => {
                self.bytes.push(0x16);
                self.bytes.push(slot as u8);
            }
        }
        self.push_stack(2);
    }

    pub fn dload(&mut self, slot: u16) {
        if slot + 1 >= self.max_locals {
            self.max_locals = slot + 2;
        }
        match slot {
            0 => self.bytes.push(0x26),
            1 => self.bytes.push(0x27),
            2 => self.bytes.push(0x28),
            3 => self.bytes.push(0x29),
            _ => {
                self.bytes.push(0x18);
                self.bytes.push(slot as u8);
            }
        }
        self.push_stack(2);
    }

    pub fn fload(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x22),
            1 => self.bytes.push(0x23),
            2 => self.bytes.push(0x24),
            3 => self.bytes.push(0x25),
            _ => {
                self.bytes.push(0x17);
                self.bytes.push(slot as u8);
            }
        }
        self.push_stack(1);
    }

    pub fn istore(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x3b),
            1 => self.bytes.push(0x3c),
            2 => self.bytes.push(0x3d),
            3 => self.bytes.push(0x3e),
            _ => {
                self.bytes.push(0x36);
                self.bytes.push(slot as u8);
            }
        }
        self.pop_stack(1);
    }

    pub fn astore(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x4b),
            1 => self.bytes.push(0x4c),
            2 => self.bytes.push(0x4d),
            3 => self.bytes.push(0x4e),
            _ => {
                self.bytes.push(0x3a);
                self.bytes.push(slot as u8);
            }
        }
        self.pop_stack(1);
    }

    pub fn lstore(&mut self, slot: u16) {
        if slot + 1 >= self.max_locals {
            self.max_locals = slot + 2;
        }
        match slot {
            0 => self.bytes.push(0x3f),
            1 => self.bytes.push(0x40),
            2 => self.bytes.push(0x41),
            3 => self.bytes.push(0x42),
            _ => {
                self.bytes.push(0x37);
                self.bytes.push(slot as u8);
            }
        }
        self.pop_stack(2);
    }

    pub fn dstore(&mut self, slot: u16) {
        if slot + 1 >= self.max_locals {
            self.max_locals = slot + 2;
        }
        match slot {
            0 => self.bytes.push(0x47),
            1 => self.bytes.push(0x48),
            2 => self.bytes.push(0x49),
            3 => self.bytes.push(0x4a),
            _ => {
                self.bytes.push(0x39);
                self.bytes.push(slot as u8);
            }
        }
        self.pop_stack(2);
    }

    pub fn fstore(&mut self, slot: u16) {
        if slot >= self.max_locals {
            self.max_locals = slot + 1;
        }
        match slot {
            0 => self.bytes.push(0x43),
            1 => self.bytes.push(0x44),
            2 => self.bytes.push(0x45),
            3 => self.bytes.push(0x46),
            _ => {
                self.bytes.push(0x38);
                self.bytes.push(slot as u8);
            }
        }
        self.pop_stack(1);
    }

    pub fn dup(&mut self) {
        self.bytes.push(0x59);
        self.push_stack(1);
    }

    pub fn dup2(&mut self) {
        self.bytes.push(0x5c);
        self.push_stack(2);
    }

    pub fn iadd(&mut self) {
        self.bytes.push(0x60);
        self.pop_stack(1);
    }

    pub fn isub(&mut self) {
        self.bytes.push(0x64);
        self.pop_stack(1);
    }

    pub fn imul(&mut self) {
        self.bytes.push(0x68);
        self.pop_stack(1);
    }

    pub fn idiv(&mut self) {
        self.bytes.push(0x6c);
        self.pop_stack(1);
    }

    pub fn irem(&mut self) {
        self.bytes.push(0x70);
        self.pop_stack(1);
    }

    pub fn ishl(&mut self) {
        self.bytes.push(0x78);
        self.pop_stack(1);
    }

    pub fn ishr(&mut self) {
        self.bytes.push(0x7a);
        self.pop_stack(1);
    }

    pub fn iushr(&mut self) {
        self.bytes.push(0x7c);
        self.pop_stack(1);
    }

    pub fn iand(&mut self) {
        self.bytes.push(0x7e);
        self.pop_stack(1);
    }

    pub fn ior(&mut self) {
        self.bytes.push(0x80);
        self.pop_stack(1);
    }

    pub fn ixor(&mut self) {
        self.bytes.push(0x82);
        self.pop_stack(1);
    }

    pub fn lshl(&mut self) {
        self.bytes.push(0x79);
        self.pop_stack(1);
    }

    pub fn lshr(&mut self) {
        self.bytes.push(0x7b);
        self.pop_stack(1);
    }

    pub fn lushr(&mut self) {
        self.bytes.push(0x7d);
        self.pop_stack(1);
    }

    pub fn land(&mut self) {
        self.bytes.push(0x7f);
        self.pop_stack(2);
    }

    pub fn lor(&mut self) {
        self.bytes.push(0x81);
        self.pop_stack(2);
    }

    pub fn lxor(&mut self) {
        self.bytes.push(0x83);
        self.pop_stack(2);
    }

    pub fn ineg(&mut self) {
        self.bytes.push(0x74);
    }

    pub fn lneg(&mut self) {
        self.bytes.push(0x75);
    }

    pub fn fneg(&mut self) {
        self.bytes.push(0x76);
    }

    pub fn dneg(&mut self) {
        self.bytes.push(0x77);
    }

    pub fn iaload(&mut self) {
        self.bytes.push(0x2e);
        self.pop_stack(1);
    }

    pub fn iastore(&mut self) {
        self.bytes.push(0x4f);
        self.pop_stack(3);
    }

    pub fn baload(&mut self) {
        self.bytes.push(0x33);
        self.pop_stack(1);
    }

    pub fn bastore(&mut self) {
        self.bytes.push(0x54);
        self.pop_stack(3);
    }

    pub fn arraylength(&mut self) {
        self.bytes.push(0xbe);
    }

    pub fn newarray(&mut self, atype: u8) {
        self.bytes.push(0xbc);
        self.bytes.push(atype);
    }

    pub fn getstatic(&mut self, cp_index: u16) {
        self.bytes.push(0xb2);
        self.bytes.extend_from_slice(&cp_index.to_be_bytes());
        self.push_stack(1);
    }

    pub fn putstatic(&mut self, cp_index: u16) {
        self.bytes.push(0xb3);
        self.bytes.extend_from_slice(&cp_index.to_be_bytes());
        self.pop_stack(1);
    }

    pub fn new_instance(&mut self, cp_class_index: u16) {
        self.bytes.push(0xbb);
        self.bytes.extend_from_slice(&cp_class_index.to_be_bytes());
        self.push_stack(1);
    }

    pub fn invokespecial(&mut self, cp_method_index: u16, arg_slots: u16) {
        self.bytes.push(0xb7);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.pop_stack(1 + arg_slots); // pop receiver + arguments
    }

    pub fn invokevirtual(&mut self, cp_method_index: u16, arg_slots: u16, has_return: bool) {
        self.bytes.push(0xb6);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.pop_stack(1 + arg_slots);
        if has_return {
            self.push_stack(1);
        }
    }

    pub fn invokestatic(&mut self, cp_method_index: u16, arg_slots: u16, has_return: bool) {
        self.bytes.push(0xb8);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.pop_stack(arg_slots);
        if has_return {
            self.push_stack(1);
        }
    }

    pub fn return_void(&mut self) {
        self.bytes.push(0xb1);
    }

    pub fn ireturn(&mut self) {
        self.bytes.push(0xac);
        self.pop_stack(1);
    }

    pub fn lreturn(&mut self) {
        self.bytes.push(0xad);
        self.pop_stack(2);
    }

    pub fn areturn(&mut self) {
        self.bytes.push(0xb0);
        self.pop_stack(1);
    }

    pub fn i2l(&mut self) {
        self.bytes.push(0x85);
        self.push_stack(1); // Long takes 2 stack words (net push 1)
    }

    pub fn l2i(&mut self) {
        self.bytes.push(0x88);
        self.pop_stack(1);
    }

    pub fn i2d(&mut self) {
        self.bytes.push(0x87);
        self.push_stack(1);
    }

    pub fn d2i(&mut self) {
        self.bytes.push(0x8e);
        self.pop_stack(1);
    }

    pub fn l2d(&mut self) {
        self.bytes.push(0x8a);
    }

    pub fn d2l(&mut self) {
        self.bytes.push(0x8f);
    }

    pub fn i2b(&mut self) {
        self.bytes.push(0x91);
    }

    pub fn i2s(&mut self) {
        self.bytes.push(0x93);
    }

    pub fn i2c(&mut self) {
        self.bytes.push(0x92);
    }

    pub fn ladd(&mut self) {
        self.bytes.push(0x61);
        self.pop_stack(2);
    }

    pub fn lsub(&mut self) {
        self.bytes.push(0x65);
        self.pop_stack(2);
    }

    pub fn lmul(&mut self) {
        self.bytes.push(0x69);
        self.pop_stack(2);
    }

    pub fn ldiv(&mut self) {
        self.bytes.push(0x6d);
        self.pop_stack(2);
    }

    pub fn lrem(&mut self) {
        self.bytes.push(0x71);
        self.pop_stack(2);
    }

    pub fn lcmp(&mut self) {
        self.bytes.push(0x94);
        self.pop_stack(3);
    }

    pub fn dadd(&mut self) {
        self.bytes.push(0x63);
        self.pop_stack(2);
    }

    pub fn dsub(&mut self) {
        self.bytes.push(0x67);
        self.pop_stack(2);
    }

    pub fn dmul(&mut self) {
        self.bytes.push(0x6b);
        self.pop_stack(2);
    }

    pub fn ddiv(&mut self) {
        self.bytes.push(0x6f);
        self.pop_stack(2);
    }

    pub fn drem(&mut self) {
        self.bytes.push(0x73);
        self.pop_stack(2);
    }

    pub fn dcmpg(&mut self) {
        self.bytes.push(0x98);
        self.pop_stack(3);
    }

    pub fn dcmpl(&mut self) {
        self.bytes.push(0x97);
        self.pop_stack(3);
    }

    pub fn dreturn(&mut self) {
        self.bytes.push(0xaf);
        self.pop_stack(2);
    }

    pub fn laload(&mut self) {
        self.bytes.push(0x2f);
    }

    pub fn lastore(&mut self) {
        self.bytes.push(0x50);
        self.pop_stack(4);
    }

    pub fn getfield(&mut self, cp_field_index: u16) {
        self.bytes.push(0xb4);
        self.bytes.extend_from_slice(&cp_field_index.to_be_bytes());
        // Pops objectref (1), pushes field value (1)
    }

    pub fn putfield(&mut self, cp_field_index: u16) {
        self.bytes.push(0xb5);
        self.bytes.extend_from_slice(&cp_field_index.to_be_bytes());
        self.pop_stack(2); // Pops objectref + value
    }

    pub fn instanceof(&mut self, cp_class_index: u16) {
        self.bytes.push(0xc1);
        self.bytes.extend_from_slice(&cp_class_index.to_be_bytes());
        // Pops objectref, pushes int result (1 or 0)
    }

    pub fn checkcast(&mut self, cp_class_index: u16) {
        self.bytes.push(0xc0);
        self.bytes.extend_from_slice(&cp_class_index.to_be_bytes());
    }

    pub fn anewarray(&mut self, cp_class_index: u16) {
        self.bytes.push(0xbd);
        self.bytes.extend_from_slice(&cp_class_index.to_be_bytes());
        // Pops count, pushes arrayref
    }

    pub fn aastore(&mut self) {
        self.bytes.push(0x53);
        self.pop_stack(3); // arrayref, index, value
    }

    pub fn aaload(&mut self) {
        self.bytes.push(0x32);
        self.pop_stack(2); // arrayref, index
        self.push_stack(1); // value
    }

    pub fn iinc(&mut self, slot: u16, delta: i8) {
        self.bytes.push(0x84);
        self.bytes.push(slot as u8);
        self.bytes.push(delta as u8);
    }

    pub fn invokeinterface(&mut self, cp_method_index: u16, arg_slots: u16, has_return: bool) {
        self.bytes.push(0xb9);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.bytes.push((arg_slots + 1) as u8); // count includes receiver
        self.bytes.push(0); // 4th byte is always 0
        self.pop_stack(1 + arg_slots);
        if has_return {
            self.push_stack(1);
        }
    }

    pub fn invokevirtual_ret(&mut self, cp_method_index: u16, arg_slots: u16, ret_slots: u16) {
        self.bytes.push(0xb6);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.pop_stack(1 + arg_slots);
        if ret_slots > 0 {
            self.push_stack(ret_slots);
        }
    }

    pub fn invokestatic_ret(&mut self, cp_method_index: u16, arg_slots: u16, ret_slots: u16) {
        self.bytes.push(0xb8);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.pop_stack(arg_slots);
        if ret_slots > 0 {
            self.push_stack(ret_slots);
        }
    }

    pub fn invokeinterface_ret(&mut self, cp_method_index: u16, arg_slots: u16, ret_slots: u16) {
        self.bytes.push(0xb9);
        self.bytes.extend_from_slice(&cp_method_index.to_be_bytes());
        self.bytes.push((arg_slots + 1) as u8);
        self.bytes.push(0);
        self.pop_stack(1 + arg_slots);
        if ret_slots > 0 {
            self.push_stack(ret_slots);
        }
    }

    pub fn getfield_slots(&mut self, cp_field_index: u16, field_slots: u16) {
        self.bytes.push(0xb4);
        self.bytes.extend_from_slice(&cp_field_index.to_be_bytes());
        self.pop_stack(1);
        self.push_stack(field_slots);
    }

    pub fn putfield_slots(&mut self, cp_field_index: u16, field_slots: u16) {
        self.bytes.push(0xb5);
        self.bytes.extend_from_slice(&cp_field_index.to_be_bytes());
        self.pop_stack(1 + field_slots);
    }

    pub fn freturn(&mut self) {
        self.bytes.push(0xae);
        self.pop_stack(1);
    }

    pub fn fconst_0(&mut self) {
        self.bytes.push(0x0b);
        self.push_stack(1);
    }

    pub fn swap(&mut self) {
        self.bytes.push(0x5f);
    }

    pub fn pop(&mut self) {
        self.bytes.push(0x57);
        self.pop_stack(1);
    }

    pub fn pop2(&mut self) {
        self.bytes.push(0x58);
        self.pop_stack(2);
    }

    pub fn emit_ifeq(&mut self) -> usize {
        self.bytes.push(0x99);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifne(&mut self) -> usize {
        self.bytes.push(0x9a);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_iflt(&mut self) -> usize {
        self.bytes.push(0x9b);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifge(&mut self) -> usize {
        self.bytes.push(0x9c);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifgt(&mut self) -> usize {
        self.bytes.push(0x9d);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifle(&mut self) -> usize {
        self.bytes.push(0x9e);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifnull(&mut self) -> usize {
        self.bytes.push(0xc6);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn emit_ifnonnull(&mut self) -> usize {
        self.bytes.push(0xc7);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(1);
        placeholder
    }

    pub fn athrow(&mut self) {
        self.bytes.push(0xbf);
        self.pop_stack(1);
    }

    pub fn emit_if_icmpeq(&mut self) -> usize {
        self.bytes.push(0x9f);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_if_icmpne(&mut self) -> usize {
        self.bytes.push(0xa0);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_if_icmplt(&mut self) -> usize {
        self.bytes.push(0xa1);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_if_icmpge(&mut self) -> usize {
        self.bytes.push(0xa2);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_if_icmpgt(&mut self) -> usize {
        self.bytes.push(0xa3);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_if_icmple(&mut self) -> usize {
        self.bytes.push(0xa4);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        self.pop_stack(2);
        placeholder
    }

    pub fn emit_goto(&mut self) -> usize {
        self.bytes.push(0xa7);
        let placeholder = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0]);
        placeholder
    }

    pub fn emit_goto_target(&mut self, target_pos: usize) {
        let opcode_pos = self.bytes.len();
        let offset = (target_pos as isize - opcode_pos as isize) as i16;
        self.bytes.push(0xa7);
        self.bytes.extend_from_slice(&offset.to_be_bytes());
    }

    pub fn patch_jump(&mut self, placeholder_pos: usize) {
        let opcode_pos = placeholder_pos - 1;
        let target_pos = self.bytes.len();
        let offset = (target_pos as isize - opcode_pos as isize) as i16;
        let bytes = offset.to_be_bytes();
        self.bytes[placeholder_pos] = bytes[0];
        self.bytes[placeholder_pos + 1] = bytes[1];
    }
}
