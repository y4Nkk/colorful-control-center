use super::*;

fn write_verified(io: &impl RpmIo, pair: &Pair) -> Result<()> {
    io.exchange(&pair.write_packet())?;
    let mut request = [0; 256];
    request[0] = pair.query;
    let actual = Pair::read(pair.query, &io.exchange(&request)?).map_err(|e| anyhow!(e))?;
    if actual != *pair {
        bail!("RPM 表 {} 回读不一致", pair.query);
    }
    Ok(())
}

pub(super) fn restore_pairs(io: &impl RpmIo, pairs: &[Pair]) -> Result<()> {
    let errors = pairs
        .iter()
        .filter_map(|pair| {
            write_verified(io, pair)
                .err()
                .map(|e| format!("恢复表 {}：{e:#}", pair.query))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

fn restore(io: &impl RpmIo, before: &Snapshot, touched: &[u8], restore_mode: bool) -> Result<()> {
    let pairs = before
        .pairs
        .iter()
        .filter(|p| touched.contains(&p.query))
        .cloned()
        .collect::<Vec<_>>();
    let tables = restore_pairs(io, &pairs);
    if !restore_mode {
        return tables;
    }
    let mode = io.restore_mode(before.mode);
    let saved = settings::apply(io, 4, &before.saved, None);
    let errors = [
        tables.err().map(|e| format!("{e:#}")),
        mode.err().map(|e| format!("{e:#}")),
        saved.err(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

pub(super) fn apply(
    io: &impl RpmIo,
    before: &Snapshot,
    targets: &[Pair],
    final_mode: Option<u8>,
) -> Result<()> {
    if final_mode.is_some_and(|mode| ![FAN_MODE_AUTO, FAN_MODE_CUSTOM].contains(&mode)) {
        bail!("未知 RPM 事务目标模式");
    }
    let verify_custom = || {
        require_custom_mode(decode_oem_fan_mode(io.fan_info()?[14]).map_err(anyhow::Error::msg)?)
    };
    if final_mode.is_none() {
        verify_custom()?;
    }
    let mut touched = Vec::new();
    let result = (|| -> Result<()> {
        if final_mode.is_some() {
            io.set_mode(FAN_MODE_CUSTOM)?;
        }
        for pair in targets {
            touched.push(pair.query);
            write_verified(io, pair)?;
        }
        let Some(mode) = final_mode else {
            return verify_custom();
        };
        if mode != FAN_MODE_CUSTOM {
            io.set_mode(mode)?;
        }
        settings::apply(io, 4, &crate::fanctl::saved::mode(io, mode)?, None)
            .map_err(|e| anyhow!(e))?;
        Ok(())
    })();
    if let Err(error) = result {
        let recovery =
            restore(io, before, &touched, final_mode.is_some()).map_err(|e| format!("{e:#}"));
        bail!(ccc_core::msr::with_recovery(format!("{error:#}"), recovery));
    }
    Ok(())
}
