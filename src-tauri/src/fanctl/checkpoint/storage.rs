use super::{FanCheckpoint, Patch};
use anyhow::{Result, anyhow, bail};
use ccc_core::{rpm::Pair, software_fan::Curves};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StoredCheckpoint {
    #[serde(deserialize_with = "Option::deserialize")]
    software: Option<Curves>,
    #[serde(deserialize_with = "Option::deserialize")]
    package: Option<Vec<u8>>,
    #[serde(deserialize_with = "Option::deserialize")]
    rpm_pairs: Option<Vec<StoredPair>>,
    fields: Vec<StoredPatch>,
    mode: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    offset_raw: Option<u8>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredPair {
    query: u8,
    tables: [Vec<u8>; 2],
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredPatch {
    offset: usize,
    data: Vec<u8>,
}

impl FanCheckpoint {
    pub(crate) fn stored(&self) -> StoredCheckpoint {
        StoredCheckpoint {
            software: self.software.clone(),
            package: self.package.map(|data| data.to_vec()),
            rpm_pairs: self.rpm_pairs.as_ref().map(|pairs| {
                pairs
                    .iter()
                    .map(|pair| StoredPair {
                        query: pair.query,
                        tables: [pair.tables[0].to_vec(), pair.tables[1].to_vec()],
                    })
                    .collect()
            }),
            fields: self
                .fields
                .iter()
                .map(|patch| StoredPatch {
                    offset: patch.offset,
                    data: patch.data.clone(),
                })
                .collect(),
            mode: self.mode,
            offset_raw: self.offset_raw,
        }
    }
}
impl StoredCheckpoint {
    pub(crate) fn restore(&self) -> Result<FanCheckpoint> {
        super::super::validate_mode(self.mode)?;
        if self.package.is_some() && self.rpm_pairs.is_some() {
            bail!("风扇恢复记录无效");
        }
        if let Some(curves) = &self.software {
            ccc_core::software_fan::validate(curves).map_err(anyhow::Error::msg)?;
        }
        let package = self
            .package
            .as_ref()
            .map(|data| {
                data.as_slice()
                    .try_into()
                    .map_err(|_| anyhow!("风扇恢复包长度无效"))
            })
            .transpose()?;
        let rpm_pairs = self
            .rpm_pairs
            .as_ref()
            .map(|pairs| parse_pairs(pairs))
            .transpose()?;
        let fields = parse_fields(&self.fields)?;
        Ok(FanCheckpoint {
            software: self.software.clone(),
            package,
            rpm_pairs,
            fields,
            mode: self.mode,
            offset_raw: self.offset_raw,
        })
    }
}
fn parse_pairs(pairs: &[StoredPair]) -> Result<Vec<Pair>> {
    if !(1..=2).contains(&pairs.len()) {
        bail!("风扇恢复表数量无效");
    }
    let mut seen = BTreeSet::new();
    pairs
        .iter()
        .map(|pair| {
            if ![40, 42].contains(&pair.query) || !seen.insert(pair.query) {
                bail!("风扇恢复表标识无效");
            }
            Ok(Pair {
                query: pair.query,
                tables: [
                    pair.tables[0]
                        .as_slice()
                        .try_into()
                        .map_err(|_| anyhow!("风扇恢复表长度无效"))?,
                    pair.tables[1]
                        .as_slice()
                        .try_into()
                        .map_err(|_| anyhow!("风扇恢复表长度无效"))?,
                ],
            })
        })
        .collect()
}
fn parse_fields(fields: &[StoredPatch]) -> Result<Vec<Patch>> {
    let quiet = super::saved::ENTERTAINMENT_QUIET_OFFSET;
    let mut allowed = vec![(5, 1), (7, 1), (quiet, 1), (70, 8)];
    for index in 0..3 {
        let (duty, temp, slope) = ccc_core::fan::oem_page4_offsets(index);
        allowed.extend([(duty, 4), (temp, 4), (slope, 6)]);
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for field in fields {
        if !allowed.contains(&(field.offset, field.data.len()))
            || !seen.insert(field.offset)
            || field.offset == quiet && field.data[0] > 1
        {
            bail!("风扇恢复字段范围或顺序无效");
        }
        result.push(Patch::new(field.offset, field.data.clone()));
    }
    if !seen.contains(&5) || !seen.contains(&7) {
        bail!("风扇恢复字段不完整");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checkpoint() -> FanCheckpoint {
        FanCheckpoint {
            software: None,
            package: Some([7; 256]),
            rpm_pairs: None,
            fields: vec![
                Patch::new(16, [1, 2, 3, 4]),
                Patch::new(5, [0]),
                Patch::new(7, [10]),
            ],
            mode: 0,
            offset_raw: Some(26),
        }
    }
    #[test]
    fn strict_round_trip_preserves_original_packet_and_fields() {
        let original = checkpoint();
        let bytes = serde_json::to_vec(&original.stored()).unwrap();
        let saved: StoredCheckpoint = serde_json::from_slice(&bytes).unwrap();
        assert!(saved.restore().unwrap() == original);
    }
    #[test]
    fn rejects_unknown_fields_invalid_offsets_lengths_and_duplicate_slots() {
        let mut saved = checkpoint().stored();
        saved.fields[0].offset = 200;
        assert!(saved.restore().is_err());
        saved = checkpoint().stored();
        saved.fields.push(saved.fields[1].clone());
        assert!(saved.restore().is_err());
        saved = checkpoint().stored();
        saved.package.as_mut().unwrap().pop();
        assert!(saved.restore().is_err());
        saved = checkpoint().stored();
        saved.rpm_pairs = Some(vec![]);
        assert!(saved.restore().is_err());
        let mut json = serde_json::to_value(checkpoint().stored()).unwrap();
        json["unknown"] = true.into();
        assert!(serde_json::from_value::<StoredCheckpoint>(json).is_err());
        let mut json = serde_json::to_value(checkpoint().stored()).unwrap();
        json.as_object_mut().unwrap().remove("offsetRaw");
        json["offset"] = 10.into();
        assert!(serde_json::from_value::<StoredCheckpoint>(json).is_err());
    }
}
