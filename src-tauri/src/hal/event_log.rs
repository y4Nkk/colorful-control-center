//! 经典Windows事件日志只读传输；不会创建来源、清空日志或回放订阅前记录。
mod native;
#[cfg(test)]
mod tests;
use anyhow::{Context, Result, bail};
pub(crate) use native::{Log, ensure_log};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    pub number: u32,
    pub event_id: u32,
    pub event_type: u16,
    pub source: String,
    pub strings: Vec<String>,
}
fn word(raw: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        raw.get(offset..offset + 4)
            .context("事件记录字段截断")?
            .try_into()?,
    ))
}
fn wide(raw: &[u8]) -> Result<(String, usize)> {
    let mut values = Vec::new();
    for pair in raw.chunks_exact(2) {
        let value = u16::from_le_bytes([pair[0], pair[1]]);
        if value == 0 {
            return Ok((String::from_utf16(&values)?, values.len() * 2 + 2));
        }
        values.push(value);
    }
    bail!("事件记录字符串缺少终止符")
}
fn decode(raw: &[u8]) -> Result<Record> {
    let end = raw.len() - 4;
    let (source, used) = wide(&raw[56..end])?;
    let (_, computer) = wide(&raw[56 + used..end])?;
    let count = u16::from_le_bytes(raw[26..28].try_into()?) as usize;
    if count > 1024 {
        bail!("事件字符串数量超过限制");
    }
    let mut strings = Vec::with_capacity(count);
    if count > 0 {
        let mut offset = word(raw, 36)? as usize;
        if offset < 56 + used + computer || offset >= end || offset % 2 != 0 {
            bail!("事件字符串偏移无效");
        }
        for _ in 0..count {
            let (value, length) = wide(raw.get(offset..end).context("事件字符串越界")?)?;
            strings.push(value);
            offset += length;
        }
    }
    Ok(Record {
        number: word(raw, 8)?,
        event_id: word(raw, 20)?,
        event_type: u16::from_le_bytes(raw[24..26].try_into()?),
        source,
        strings,
    })
}
pub(crate) fn parse(raw: &[u8], cursor: u32) -> Result<(u32, Vec<Record>)> {
    let mut remaining = raw;
    let mut next = cursor;
    let mut records = Vec::new();
    while !remaining.is_empty() {
        let length = word(remaining, 0)? as usize;
        if records.len() >= 1024 || length < 64 || length > remaining.len() || length % 4 != 0 {
            bail!("事件记录长度或数量无效");
        }
        let record = &remaining[..length];
        if word(record, 4)? != 0x654c664c || word(record, length - 4)? != length as u32 {
            bail!("事件记录标记无效");
        }
        if word(record, 8)? != next {
            bail!("事件记录出现缺口或重放");
        }
        next = next.checked_add(1).context("事件序号溢出")?;
        records.push(decode(record)?);
        remaining = &remaining[length..];
    }
    Ok((next, records))
}
