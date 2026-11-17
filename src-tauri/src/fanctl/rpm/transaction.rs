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

fn restore(io: &impl RpmIo, before: &Snapshot, touched: &[u8]) -> Result<()> {
    let pairs = before
        .pairs
        .iter()
        .filter(|p| touched.contains(&p.query))
        .cloned()
        .collect::<Vec<_>>();
    let tables = restore_pairs(io, &pairs);
    let mode = io.mode(before.mode);
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
    final_mode: u8,
) -> Result<()> {
    if ![0, 6].contains(&final_mode) {
        bail!("未知 RPM 事务目标模式");
    }
    let mut touched = Vec::new();
    let result = (|| -> Result<()> {
        io.mode(6)?;
        for pair in targets {
            touched.push(pair.query);
            write_verified(io, pair)?;
        }
        if final_mode != 6 {
            io.mode(final_mode)?;
        }
        settings::apply(io, 4, &crate::fanctl::saved::mode(io, final_mode)?, None)
            .map_err(|e| anyhow!(e))?;
        Ok(())
    })();
    if let Err(error) = result {
        let recovery = restore(io, before, &touched).map_err(|e| format!("{e:#}"));
        bail!(ccc_core::msr::with_recovery(format!("{error:#}"), recovery));
    }
    Ok(())
}
