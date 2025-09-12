use bytes::{Buf, BufMut, BytesMut};
use crc32fast::Hasher;
use std::fmt;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpType {
    Set = 0,
    Del = 1,
    Incr = 2,
    Cas = 3, // Compare-and-swap
}

impl OpType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(OpType::Set),
            1 => Some(OpType::Del),
            2 => Some(OpType::Incr),
            3 => Some(OpType::Cas),
            _ => None,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

#[derive(Debug, Clone)]
pub struct WalEntry {
    pub timestamp: u64,      // Unix nanos
    pub key: String,
    pub value: Vec<u8>,      // empty for DEL
    pub version: u64,        // for CAS/MVCC later
    pub ttl: Option<u64>,    // Unix nanos or 0 for none
    pub op_type: OpType,
}

impl WalEntry {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = BytesMut::new();

        // Fixed-size header: 8+8+8+1+8 = 33 bytes
        buf.put_u64(self.timestamp);
        buf.put_u64(self.version);
        buf.put_u64(self.ttl.unwrap_or(0)); // 0 = no TTL
        buf.put_u8(self.op_type.as_u8());
        buf.put_u64(self.key.len() as u64);
        buf.put_u64(self.value.len() as u64);

        // Variable data
        buf.put(self.key.as_bytes());
        buf.put(&self.value[..]);

        // Calculate checksum over entire payload (excluding checksum itself)
        let mut hasher = Hasher::new();
        hasher.update(&buf);
        let checksum = hasher.finalize();

        // Append checksum (4 bytes)
        buf.put_u32(checksum);

        buf.to_vec()
    }

    pub fn deserialize( &[u8]) -> Result<(Self, usize), WalError> {
        if data.len() < 37 { // min header + checksum
            return Err(WalError::InvalidEntry {
                offset: 0,
                reason: "too short".to_string(),
            });
        }

        let mut offset = 0;

        let timestamp = read_u64(data, &mut offset)?;
        let version = read_u64(data, &mut offset)?;
        let ttl_raw = read_u64(data, &mut offset)?;
        let op_byte = read_u8(data, &mut offset)?;
        let key_len = read_u64(data, &mut offset)? as usize;
        let value_len = read_u64(data, &mut offset)? as usize;

        if data.len() < offset + key_len + value_len + 4 {
            return Err(WalError::InvalidEntry {
                offset: 0,
                reason: "incomplete data".to_string(),
            });
        }

        let key = std::str::from_utf8(&data[offset..offset + key_len])
            .map_err(|_| WalError::InvalidEntry {
                offset: 0,
                reason: "invalid UTF-8 key".to_string(),
            })?
            .to_string();
        offset += key_len;

        let value = data[offset..offset + value_len].to_vec();
        offset += value_len;

        let checksum_stored = read_u32(data, &mut offset)?;

        // Verify checksum
        let mut hasher = Hasher::new();
        hasher.update(&data[..offset - 4]); // everything before checksum
        let checksum_computed = hasher.finalize();

        if checksum_stored != checksum_computed {
            return Err(WalError::ChecksumMismatch {
                offset: 0,
                expected: checksum_computed,
                got: checksum_stored,
            });
        }

        let ttl = if ttl_raw == 0 { None } else { Some(ttl_raw) };

        let op_type = OpType::from_u8(op_byte).ok_or_else(|| WalError::InvalidEntry {
            offset: 0,
            reason: format!("unknown op type: {}", op_byte),
        })?;

        Ok((
            WalEntry {
                timestamp,
                key,
                value,
                version,
                ttl,
                op_type,
            },
            offset,
        ))
    }
}

// Helper functions
fn read_u64(data: &[u8], offset: &mut usize) -> Result<u64, WalError> {
    if *offset + 8 > data.len() {
        return Err(WalError::InvalidEntry {
            offset: *offset as u64,
            reason: "unexpected EOF".to_string(),
        });
    }
    let val = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    Ok(val)
}

fn read_u32(data: &[u8], offset: &mut usize) -> Result<u32, WalError> {
    if *offset + 4 > data.len() {
        return Err(WalError::InvalidEntry {
            offset: *offset as u64,
            reason: "unexpected EOF".to_string(),
        });
    }
    let val = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    Ok(val)
}

fn read_u8( &[u8], offset: &mut usize) -> Result<u8, WalError> {
    if *offset >= data.len() {
        return Err(WalError::InvalidEntry {
            offset: *offset as u64,
            reason: "unexpected EOF".to_string(),
        });
    }
    let val = data[*offset];
    *offset += 1;
    Ok(val)
}