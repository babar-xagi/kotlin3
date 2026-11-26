use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum CpEntry {
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Dummy,
    Class(u16),
    String(u16),
    Fieldref { class_index: u16, name_and_type_index: u16 },
    Methodref { class_index: u16, name_and_type_index: u16 },
    InterfaceMethodref { class_index: u16, name_and_type_index: u16 },
    NameAndType { name_index: u16, descriptor_index: u16 },
}

#[derive(Debug, Clone)]
pub struct ConstantPool {
    entries: Vec<CpEntry>,
    utf8_map: HashMap<String, u16>,
    int_map: HashMap<i32, u16>,
    class_map: HashMap<u16, u16>,
    string_map: HashMap<u16, u16>,
    nat_map: HashMap<(u16, u16), u16>,
    method_map: HashMap<(u16, u16), u16>,
    field_map: HashMap<(u16, u16), u16>,
    long_map: HashMap<i64, u16>,
    double_map: HashMap<u64, u16>,
}

impl ConstantPool {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            utf8_map: HashMap::new(),
            int_map: HashMap::new(),
            class_map: HashMap::new(),
            string_map: HashMap::new(),
            nat_map: HashMap::new(),
            method_map: HashMap::new(),
            field_map: HashMap::new(),
            long_map: HashMap::new(),
            double_map: HashMap::new(),
        }
    }

    pub fn add_utf8(&mut self, s: &str) -> u16 {
        if let Some(&idx) = self.utf8_map.get(s) {
            return idx;
        }
        self.entries.push(CpEntry::Utf8(s.to_string()));
        let idx = self.entries.len() as u16;
        self.utf8_map.insert(s.to_string(), idx);
        idx
    }

    pub fn add_class(&mut self, internal_name: &str) -> u16 {
        let name_idx = self.add_utf8(internal_name);
        if let Some(&idx) = self.class_map.get(&name_idx) {
            return idx;
        }
        self.entries.push(CpEntry::Class(name_idx));
        let idx = self.entries.len() as u16;
        self.class_map.insert(name_idx, idx);
        idx
    }

    pub fn add_string(&mut self, s: &str) -> u16 {
        let str_idx = self.add_utf8(s);
        if let Some(&idx) = self.string_map.get(&str_idx) {
            return idx;
        }
        self.entries.push(CpEntry::String(str_idx));
        let idx = self.entries.len() as u16;
        self.string_map.insert(str_idx, idx);
        idx
    }

    pub fn add_name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let n_idx = self.add_utf8(name);
        let d_idx = self.add_utf8(descriptor);
        if let Some(&idx) = self.nat_map.get(&(n_idx, d_idx)) {
            return idx;
        }
        self.entries.push(CpEntry::NameAndType {
            name_index: n_idx,
            descriptor_index: d_idx,
        });
        let idx = self.entries.len() as u16;
        self.nat_map.insert((n_idx, d_idx), idx);
        idx
    }

    pub fn add_methodref(&mut self, class_name: &str, name: &str, descriptor: &str) -> u16 {
        let c_idx = self.add_class(class_name);
        let nat_idx = self.add_name_and_type(name, descriptor);
        if let Some(&idx) = self.method_map.get(&(c_idx, nat_idx)) {
            return idx;
        }
        self.entries.push(CpEntry::Methodref {
            class_index: c_idx,
            name_and_type_index: nat_idx,
        });
        let idx = self.entries.len() as u16;
        self.method_map.insert((c_idx, nat_idx), idx);
        idx
    }

    pub fn add_fieldref(&mut self, class_name: &str, name: &str, descriptor: &str) -> u16 {
        let c_idx = self.add_class(class_name);
        let nat_idx = self.add_name_and_type(name, descriptor);
        if let Some(&idx) = self.field_map.get(&(c_idx, nat_idx)) {
            return idx;
        }
        self.entries.push(CpEntry::Fieldref {
            class_index: c_idx,
            name_and_type_index: nat_idx,
        });
        let idx = self.entries.len() as u16;
        self.field_map.insert((c_idx, nat_idx), idx);
        idx
    }

    pub fn add_interface_methodref(&mut self, class_name: &str, name: &str, descriptor: &str) -> u16 {
        let c_idx = self.add_class(class_name);
        let nat_idx = self.add_name_and_type(name, descriptor);
        self.entries.push(CpEntry::InterfaceMethodref {
            class_index: c_idx,
            name_and_type_index: nat_idx,
        });
        self.entries.len() as u16
    }

    pub fn add_integer(&mut self, val: i32) -> u16 {
        if let Some(&idx) = self.int_map.get(&val) {
            return idx;
        }
        let idx = (self.entries.len() + 1) as u16;
        self.entries.push(CpEntry::Integer(val));
        self.int_map.insert(val, idx);
        idx
    }

    pub fn add_long(&mut self, val: i64) -> u16 {
        if let Some(&idx) = self.long_map.get(&val) {
            return idx;
        }
        let idx = (self.entries.len() + 1) as u16;
        self.entries.push(CpEntry::Long(val));
        self.entries.push(CpEntry::Dummy);
        self.long_map.insert(val, idx);
        idx
    }

    pub fn add_double(&mut self, val: f64) -> u16 {
        let bits = val.to_bits();
        if let Some(&idx) = self.double_map.get(&bits) {
            return idx;
        }
        let idx = (self.entries.len() + 1) as u16;
        self.entries.push(CpEntry::Double(val));
        self.entries.push(CpEntry::Dummy);
        self.double_map.insert(bits, idx);
        idx
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Constant pool count is entries + 1 (1-indexed in JVM)
        let count = (self.entries.len() + 1) as u16;
        bytes.extend_from_slice(&count.to_be_bytes());

        for entry in &self.entries {
            match entry {
                CpEntry::Utf8(s) => {
                    bytes.push(1); // Tag 1: Utf8
                    let str_bytes = s.as_bytes();
                    let len = str_bytes.len() as u16;
                    bytes.extend_from_slice(&len.to_be_bytes());
                    bytes.extend_from_slice(str_bytes);
                }
                CpEntry::Integer(val) => {
                    bytes.push(3); // Tag 3: Integer
                    bytes.extend_from_slice(&val.to_be_bytes());
                }
                CpEntry::Float(val) => {
                    bytes.push(4); // Tag 4: Float
                    bytes.extend_from_slice(&val.to_be_bytes());
                }
                CpEntry::Long(val) => {
                    bytes.push(5); // Tag 5: Long
                    bytes.extend_from_slice(&val.to_be_bytes());
                }
                CpEntry::Double(val) => {
                    bytes.push(6); // Tag 6: Double
                    bytes.extend_from_slice(&val.to_be_bytes());
                }
                CpEntry::Dummy => {}
                CpEntry::Class(name_idx) => {
                    bytes.push(7); // Tag 7: Class
                    bytes.extend_from_slice(&name_idx.to_be_bytes());
                }
                CpEntry::String(str_idx) => {
                    bytes.push(8); // Tag 8: String
                    bytes.extend_from_slice(&str_idx.to_be_bytes());
                }
                CpEntry::Fieldref {
                    class_index,
                    name_and_type_index,
                } => {
                    bytes.push(9); // Tag 9: Fieldref
                    bytes.extend_from_slice(&class_index.to_be_bytes());
                    bytes.extend_from_slice(&name_and_type_index.to_be_bytes());
                }
                CpEntry::Methodref {
                    class_index,
                    name_and_type_index,
                } => {
                    bytes.push(10); // Tag 10: Methodref
                    bytes.extend_from_slice(&class_index.to_be_bytes());
                    bytes.extend_from_slice(&name_and_type_index.to_be_bytes());
                }
                CpEntry::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                } => {
                    bytes.push(11); // Tag 11: InterfaceMethodref
                    bytes.extend_from_slice(&class_index.to_be_bytes());
                    bytes.extend_from_slice(&name_and_type_index.to_be_bytes());
                }
                CpEntry::NameAndType {
                    name_index,
                    descriptor_index,
                } => {
                    bytes.push(12); // Tag 12: NameAndType
                    bytes.extend_from_slice(&name_index.to_be_bytes());
                    bytes.extend_from_slice(&descriptor_index.to_be_bytes());
                }
            }
        }
        bytes
    }
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub code: Option<CodeAttr>,
}

#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct CodeAttr {
    pub max_stack: u16,
    pub max_locals: u16,
    pub bytecode: Vec<u8>,
    pub exception_table: Vec<ExceptionHandler>,
}

impl CodeAttr {
    pub fn new(max_stack: u16, max_locals: u16, bytecode: Vec<u8>) -> Self {
        Self {
            max_stack,
            max_locals,
            bytecode,
            exception_table: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
}

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub cp: ConstantPool,
    pub access_flags: u16,
    pub this_class_name: String,
    pub super_class_name: String,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub source_file: Option<String>,
}

impl ClassFile {
    pub fn new(this_class: &str, super_class: &str) -> Self {
        let mut cp = ConstantPool::new();
        let _ = cp.add_class(this_class);
        let _ = cp.add_class(super_class);

        Self {
            minor_version: 0,
            major_version: 49, // Java 5 baseline: universal compatibility without requiring StackMapTable
            cp,
            access_flags: 0x0001 | 0x0020, // ACC_PUBLIC | ACC_SUPER
            this_class_name: this_class.to_string(),
            super_class_name: super_class.to_string(),
            interfaces: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            source_file: None,
        }
    }

    pub fn add_interface(&mut self, interface_name: &str) {
        self.interfaces.push(interface_name.to_string());
    }

    pub fn add_field(&mut self, access_flags: u16, name: &str, descriptor: &str) {
        let name_idx = self.cp.add_utf8(name);
        let desc_idx = self.cp.add_utf8(descriptor);
        self.fields.push(FieldInfo {
            access_flags,
            name_index: name_idx,
            descriptor_index: desc_idx,
        });
    }

    pub fn add_method(
        &mut self,
        access_flags: u16,
        name: &str,
        descriptor: &str,
        code: Option<CodeAttr>,
    ) {
        let name_idx = self.cp.add_utf8(name);
        let desc_idx = self.cp.add_utf8(descriptor);
        self.methods.push(MethodInfo {
            access_flags,
            name_index: name_idx,
            descriptor_index: desc_idx,
            code,
        });
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        // Ensure classes and attributes are in constant pool
        let this_class_idx = self.cp.add_class(&self.this_class_name);
        let super_class_idx = self.cp.add_class(&self.super_class_name);
        let code_utf8_idx = self.cp.add_utf8("Code");
        let source_utf8_idx = self.source_file.as_ref().map(|_| self.cp.add_utf8("SourceFile"));
        let source_val_idx = self.source_file.as_ref().map(|s| self.cp.add_utf8(s));

        let mut body_bytes = Vec::new();
        body_bytes.extend_from_slice(&self.access_flags.to_be_bytes());
        body_bytes.extend_from_slice(&this_class_idx.to_be_bytes());
        body_bytes.extend_from_slice(&super_class_idx.to_be_bytes());

        // Interfaces
        let interfaces_count = self.interfaces.len() as u16;
        body_bytes.extend_from_slice(&interfaces_count.to_be_bytes());
        for iface in &self.interfaces {
            let iface_idx = self.cp.add_class(iface);
            body_bytes.extend_from_slice(&iface_idx.to_be_bytes());
        }

        // Fields
        let fields_count = self.fields.len() as u16;
        body_bytes.extend_from_slice(&fields_count.to_be_bytes());
        for field in &self.fields {
            body_bytes.extend_from_slice(&field.access_flags.to_be_bytes());
            body_bytes.extend_from_slice(&field.name_index.to_be_bytes());
            body_bytes.extend_from_slice(&field.descriptor_index.to_be_bytes());
            body_bytes.extend_from_slice(&0u16.to_be_bytes()); // attributes count = 0
        }

        // Methods
        let methods_count = self.methods.len() as u16;
        body_bytes.extend_from_slice(&methods_count.to_be_bytes());

        for method in &self.methods {
            body_bytes.extend_from_slice(&method.access_flags.to_be_bytes());
            body_bytes.extend_from_slice(&method.name_index.to_be_bytes());
            body_bytes.extend_from_slice(&method.descriptor_index.to_be_bytes());

            if let Some(code) = &method.code {
                // Attributes count = 1 (Code attribute)
                body_bytes.extend_from_slice(&1u16.to_be_bytes());
                body_bytes.extend_from_slice(&code_utf8_idx.to_be_bytes());

                // Attribute length = 2 (max_stack) + 2 (max_locals) + 4 (code_length) + code.len() + 2 (exception_table_length) + 8*handlers + 2 (attributes_count)
                let exc_bytes = code.exception_table.len() * 8;
                let attr_len = (2 + 2 + 4 + code.bytecode.len() + 2 + exc_bytes + 2) as u32;
                body_bytes.extend_from_slice(&attr_len.to_be_bytes());
                body_bytes.extend_from_slice(&code.max_stack.to_be_bytes());
                body_bytes.extend_from_slice(&code.max_locals.to_be_bytes());

                let code_len = code.bytecode.len() as u32;
                body_bytes.extend_from_slice(&code_len.to_be_bytes());
                body_bytes.extend_from_slice(&code.bytecode);

                let exc_count = code.exception_table.len() as u16;
                body_bytes.extend_from_slice(&exc_count.to_be_bytes());
                for handler in &code.exception_table {
                    body_bytes.extend_from_slice(&handler.start_pc.to_be_bytes());
                    body_bytes.extend_from_slice(&handler.end_pc.to_be_bytes());
                    body_bytes.extend_from_slice(&handler.handler_pc.to_be_bytes());
                    body_bytes.extend_from_slice(&handler.catch_type.to_be_bytes());
                }
                // Code sub-attributes count = 0
                body_bytes.extend_from_slice(&0u16.to_be_bytes());
            } else {
                body_bytes.extend_from_slice(&0u16.to_be_bytes());
            }
        }

        // Class Attributes (SourceFile)
        if let (Some(attr_name_idx), Some(file_name_idx)) = (source_utf8_idx, source_val_idx) {
            body_bytes.extend_from_slice(&1u16.to_be_bytes()); // Attributes count = 1
            body_bytes.extend_from_slice(&attr_name_idx.to_be_bytes());
            body_bytes.extend_from_slice(&2u32.to_be_bytes()); // Attribute length = 2
            body_bytes.extend_from_slice(&file_name_idx.to_be_bytes());
        } else {
            body_bytes.extend_from_slice(&0u16.to_be_bytes()); // Attributes count = 0
        }

        let mut class_bytes = Vec::new();
        // Magic
        class_bytes.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
        // Minor & Major Version
        class_bytes.extend_from_slice(&self.minor_version.to_be_bytes());
        class_bytes.extend_from_slice(&self.major_version.to_be_bytes());
        // Constant Pool
        class_bytes.extend_from_slice(&self.cp.to_bytes());
        // Rest of the classfile
        class_bytes.extend_from_slice(&body_bytes);

        class_bytes
    }
}
