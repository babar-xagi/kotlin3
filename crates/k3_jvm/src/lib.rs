pub struct ClassFile {
    pub magic: u32,
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: Vec<CpInfo>,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CpInfo {
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class { name_index: u16 },
    String { string_index: u16 },
    Fieldref { class_index: u16, name_and_type_index: u16 },
    Methodref { class_index: u16, name_and_type_index: u16 },
    NameAndType { name_index: u16, descriptor_index: u16 },
}

impl Default for ClassFile {
    fn default() -> Self {
        Self {
            magic: 0xCAFEBABE,
            minor_version: 0,
            major_version: 65, // Java 21 LTS bytecode
            constant_pool: vec![],
            access_flags: 0x0001 | 0x0020, // ACC_PUBLIC | ACC_SUPER
            this_class: 0,
            super_class: 0,
        }
    }
}

impl ClassFile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.magic.to_be_bytes());
        bytes.extend_from_slice(&self.minor_version.to_be_bytes());
        bytes.extend_from_slice(&self.major_version.to_be_bytes());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_magic() {
        let cf = ClassFile::new();
        let bytes = cf.to_bytes();
        assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }
}
