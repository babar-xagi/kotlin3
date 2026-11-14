pub struct JarBuilder {
    entries: Vec<ZipEntry>,
}

struct ZipEntry {
    name: String,
    data: Vec<u8>,
    crc32: u32,
    offset: u32,
}

impl JarBuilder {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_file(&mut self, name: &str, data: Vec<u8>) {
        let crc = compute_crc32(&data);
        self.entries.push(ZipEntry {
            name: name.to_string(),
            data,
            crc32: crc,
            offset: 0,
        });
    }

    pub fn set_main_class(&mut self, main_class: &str) {
        let manifest = format!(
            "Manifest-Version: 1.0\r\nMain-Class: {}\r\nCreated-By: Kotlin3 (k3)\r\n\r\n",
            main_class.replace('/', ".")
        );
        self.add_file("META-INF/MANIFEST.MF", manifest.into_bytes());
    }

    pub fn build(mut self) -> Vec<u8> {
        let mut zip_bytes = Vec::new();

        // 1. Write Local File Headers and Data
        for entry in &mut self.entries {
            entry.offset = zip_bytes.len() as u32;

            let name_bytes = entry.name.as_bytes();
            let name_len = name_bytes.len() as u16;
            let data_len = entry.data.len() as u32;

            zip_bytes.extend_from_slice(&0x04034b50u32.to_le_bytes()); // Local header signature
            zip_bytes.extend_from_slice(&20u16.to_le_bytes());         // Version needed
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // General purpose bit flag
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Compression (0 = Stored)
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Mod time
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Mod date
            zip_bytes.extend_from_slice(&entry.crc32.to_le_bytes());   // CRC-32
            zip_bytes.extend_from_slice(&data_len.to_le_bytes());      // Compressed size
            zip_bytes.extend_from_slice(&data_len.to_le_bytes());      // Uncompressed size
            zip_bytes.extend_from_slice(&name_len.to_le_bytes());      // File name length
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Extra field length
            zip_bytes.extend_from_slice(name_bytes);                   // File name
            zip_bytes.extend_from_slice(&entry.data);                  // File data
        }

        // 2. Write Central Directory
        let cd_offset = zip_bytes.len() as u32;

        for entry in &self.entries {
            let name_bytes = entry.name.as_bytes();
            let name_len = name_bytes.len() as u16;
            let data_len = entry.data.len() as u32;

            zip_bytes.extend_from_slice(&0x02014b50u32.to_le_bytes()); // Central directory signature
            zip_bytes.extend_from_slice(&20u16.to_le_bytes());         // Version made by
            zip_bytes.extend_from_slice(&20u16.to_le_bytes());         // Version needed
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Bit flag
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Compression
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Mod time
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Mod date
            zip_bytes.extend_from_slice(&entry.crc32.to_le_bytes());   // CRC-32
            zip_bytes.extend_from_slice(&data_len.to_le_bytes());      // Compressed size
            zip_bytes.extend_from_slice(&data_len.to_le_bytes());      // Uncompressed size
            zip_bytes.extend_from_slice(&name_len.to_le_bytes());      // Name length
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Extra field len
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Comment len
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Disk start
            zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Internal attrs
            zip_bytes.extend_from_slice(&0u32.to_le_bytes());          // External attrs
            zip_bytes.extend_from_slice(&entry.offset.to_le_bytes());  // Offset of local header
            zip_bytes.extend_from_slice(name_bytes);                   // File name
        }

        let cd_size = (zip_bytes.len() as u32) - cd_offset;
        let entries_count = self.entries.len() as u16;

        // 3. End of Central Directory Record (EOCD)
        zip_bytes.extend_from_slice(&0x06054b50u32.to_le_bytes()); // EOCD signature
        zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Disk number
        zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Disk with CD
        zip_bytes.extend_from_slice(&entries_count.to_le_bytes()); // Entries on this disk
        zip_bytes.extend_from_slice(&entries_count.to_le_bytes()); // Total entries
        zip_bytes.extend_from_slice(&cd_size.to_le_bytes());       // Size of CD
        zip_bytes.extend_from_slice(&cd_offset.to_le_bytes());     // Offset of CD
        zip_bytes.extend_from_slice(&0u16.to_le_bytes());          // Comment len

        zip_bytes
    }
}

fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}
