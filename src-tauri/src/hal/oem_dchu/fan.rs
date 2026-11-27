use super::Dchu;
use anyhow::{Context, Result, anyhow, bail, ensure};
use ccc_core::fan::{decode_oem_fan_mode, oem_fan_mode_bits};
use std::fmt;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub(super) const MODE_CONFIRM_TIMEOUT: Duration = Duration::from_millis(500);
const MODE_POLL_INTERVAL: Duration = Duration::from_millis(100);

struct ModeReadback {
    started: Instant,
    samples: Vec<(Duration, u8)>,
}

impl ModeReadback {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            samples: Vec::new(),
        }
    }

    fn read(&mut self, io: &Dchu) -> Result<u8> {
        let bits = io.read_fan_mode_bits().with_context(|| self.to_string())?;
        self.samples.push((self.started.elapsed(), bits));
        Ok(bits)
    }

    fn poll_after_command(
        &mut self,
        io: &Dchu,
        expected: u8,
        verify_at_window_end: bool,
    ) -> Result<u8> {
        let window = Instant::now();
        let mut previous = None;
        loop {
            let bits = self.read(io)?;
            let remaining = MODE_CONFIRM_TIMEOUT.saturating_sub(window.elapsed());
            if bits == expected
                && previous == Some(expected)
                && (!verify_at_window_end || remaining.is_zero())
            {
                return Ok(bits);
            }
            let wait = if remaining.is_zero() {
                if previous.is_some() && bits != expected {
                    return Ok(bits);
                }
                // 首读可能已耗尽窗口，窗口末也可能才首次读到目标。两者都需独立复核。
                MODE_POLL_INTERVAL
            } else {
                remaining.min(MODE_POLL_INTERVAL)
            };
            previous = Some(bits);
            std::thread::sleep(wait);
        }
    }
}

impl fmt::Display for ModeReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} 次回读，历时 {} ms，cmd13[14] [",
            self.samples.len(),
            self.started.elapsed().as_millis()
        )?;
        for (index, (elapsed, bits)) in self.samples.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}ms={bits:#04x}", elapsed.as_millis())?;
        }
        write!(f, "]")
    }
}

impl Dchu {
    pub fn read_fan_info(&self) -> Result<[u8; 256]> {
        let info = self.read_package(13)?;
        ensure!((1..=4).contains(&info[12]), "EC 风扇能力包未报告有效通道数");
        Ok(info)
    }

    fn read_fan_mode_bits(&self) -> Result<u8> {
        Ok(self.read_fan_info()?[14])
    }

    pub fn read_fan_mode(&self) -> Result<u8> {
        decode_oem_fan_mode(self.read_fan_mode_bits()?).map_err(anyhow::Error::msg)
    }

    pub(crate) fn wait_preset_fan_mode(&self) -> Result<()> {
        let mut readback = ModeReadback::new();
        loop {
            let bits = readback.read(self).inspect_err(|_| {
                self.fan_mode_unconfirmed.store(true, Ordering::Relaxed);
            })?;
            let reason = match decode_oem_fan_mode(bits) {
                Ok(mode) => {
                    if readback.samples.len() > 1 {
                        log::info!("[fan-mode] 预设命令后的回读已确认档位 {mode}，{readback}");
                    }
                    return Ok(());
                }
                Err(reason) => reason,
            };
            let remaining = MODE_CONFIRM_TIMEOUT.saturating_sub(readback.started.elapsed());
            if remaining.is_zero() {
                self.fan_mode_unconfirmed.store(true, Ordering::Relaxed);
                bail!("{reason}（{readback}）");
            }
            std::thread::sleep(remaining.min(MODE_POLL_INTERVAL));
        }
    }

    pub fn set_fan_mode_verified(&self, mode: u8) -> Result<()> {
        self.confirm_fan_mode(mode, false)
    }

    pub(crate) fn force_restore_fan_mode(&self, mode: u8) -> Result<()> {
        self.confirm_fan_mode(mode, true)
    }

    fn confirm_fan_mode(&self, mode: u8, force_restore: bool) -> Result<()> {
        let expected = oem_fan_mode_bits(mode).ok_or_else(|| anyhow!("不支持此风扇档位 {mode}"))?;
        let was_unconfirmed = self.fan_mode_unconfirmed.swap(true, Ordering::Relaxed);
        let phase = if force_restore {
            "恢复命令后的回读"
        } else {
            "风扇命令后的回读"
        };
        self.send_fan_mode(mode, expected, force_restore || was_unconfirmed, phase)
            .inspect_err(|error| log::warn!("[fan-mode] {phase}未确认档位 {mode}：{error:#}"))
    }

    fn send_fan_mode(
        &self,
        mode: u8,
        expected: u8,
        verify_at_window_end: bool,
        phase: &str,
    ) -> Result<()> {
        let mut readback = ModeReadback::new();
        let mut writes = 1;
        loop {
            self.set_data_sub(121, 1, u32::from(mode))
                .with_context(|| format!("发送风扇档位 {mode} 第 {writes} 次失败（{readback}）"))?;
            let prior_reads = readback.samples.len();
            let bits = readback
                .poll_after_command(self, expected, verify_at_window_end)
                .with_context(|| format!("风扇档位 {mode} 第 {writes} 次发送后读取失败"))?;
            let samples = &readback.samples[prior_reads..];
            if bits == expected {
                self.fan_mode_unconfirmed.store(false, Ordering::Relaxed);
                if writes > 1 || samples.iter().any(|(_, sample)| *sample != expected) {
                    log::info!("[fan-mode] {phase}经 {writes} 次发送确认档位 {mode}，{readback}");
                }
                return Ok(());
            }
            let actual = decode_oem_fan_mode(bits);
            if writes == 1 && actual.is_ok() && samples.iter().all(|(_, sample)| *sample == bits) {
                writes += 1;
                continue;
            }
            let actual = actual
                .map(|mode| format!("EC 风扇档位回读为 {mode}"))
                .unwrap_or_else(|reason| reason);
            bail!("{actual}，目标为 {mode}（{writes} 次发送，预期 {expected:#04x}，{readback}）");
        }
    }

    pub(crate) fn restore_fan_mode(&self, mode: u8) -> Result<()> {
        if self.fan_mode_unconfirmed.load(Ordering::Relaxed) {
            return self.force_restore_fan_mode(mode);
        }
        match self.read_fan_mode() {
            Ok(current) if current == mode => Ok(()),
            Ok(_) => self.force_restore_fan_mode(mode),
            Err(error) => self
                .force_restore_fan_mode(mode)
                .with_context(|| format!("恢复前读取即时风扇档位失败，{error:#}")),
        }
    }

    pub fn read_fan_offset(&self, reviewed_dsdt: bool) -> Result<u8> {
        Ok(self.fan_window(3, reviewed_dsdt)?[0xe7])
    }

    pub fn set_fan_offset_verified(&self, reviewed_dsdt: bool, raw: u8) -> Result<()> {
        ensure!(reviewed_dsdt, "当前固件未确认 EC 风扇偏移回读通道");
        self.set_data_sub(121, 14, u32::from(raw))?;
        let actual = self.read_fan_offset(reviewed_dsdt)?;
        ensure!(actual == raw, "EC 风扇偏移回读为 {actual}，目标为 {raw}");
        Ok(())
    }

    pub(crate) fn read_fan_curve_package(
        &self,
        reviewed_dsdt: bool,
        info: &[u8; 256],
        channels: u8,
    ) -> Result<[u8; 256]> {
        let window = self.fan_window(5, reviewed_dsdt)?;
        curve_package(&window, info, channels)
    }

    fn fan_window(&self, selector: u8, reviewed_dsdt: bool) -> Result<[u8; 256]> {
        ensure!(reviewed_dsdt, "当前固件未确认 EC 风扇只读窗口");
        let mut request = [0; 256];
        request[0] = selector;
        let data = self.exchange_package(4, &request)?;
        if data.iter().all(|&b| b == 0) || data.iter().all(|&b| b == 0xff) {
            return Err(anyhow!("EC 风扇只读窗口返回空数据"));
        }
        Ok(data)
    }
}

fn curve_package(window: &[u8; 256], info: &[u8; 256], channels: u8) -> Result<[u8; 256]> {
    ensure!(window[0x0a] == info[12], "EC 两点曲线窗口的风扇数量不一致");
    let mut package = [0; 256];
    for index in 0..3 {
        let ec = 0x0c + index * 14;
        let at = 16 + index * 8;
        let points = [
            window[ec + 1],
            window[ec + 5],
            window[ec + 2],
            window[ec + 6],
        ];
        if channels & (1 << index) != 0 {
            ensure!(
                window[ec] == info[at]
                    && window[ec + 4] == info[at + 1]
                    && info[at + 2..at + 6] == points,
                "EC 风扇 {} 的曲线回读不一致",
                index + 1
            );
        }
        let p = 2 + index * 4;
        let s = 14 + index * 6;
        package[p..p + 4].copy_from_slice(&points);
        package[s..s + 6].copy_from_slice(&window[ec + 8..ec + 14]);
    }
    Ok(package)
}
