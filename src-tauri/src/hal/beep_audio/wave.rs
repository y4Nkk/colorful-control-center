#[cfg(not(test))]
use anyhow::{Result, ensure};
#[cfg(not(test))]
use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_LOOP, SND_MEMORY, SND_NODEFAULT, SND_SYNC},
    core::PCWSTR,
};

const fn silent_wave() -> [u8; 90156] {
    let header: &[u8; 44] = b"RIFF\x24\x60\x01\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\0\x60\x01\0";
    let mut data = [0; 90156];
    let mut index = 0;
    while index < header.len() {
        data[index] = header[index];
        index += 1;
    }
    data
}
static SILENCE: [u8; 90156] = silent_wave();

#[cfg(not(test))]
pub(crate) fn silence(enabled: bool) -> Result<()> {
    let result = unsafe {
        if enabled {
            PlaySoundW(
                PCWSTR(SILENCE.as_ptr().cast()),
                None,
                SND_MEMORY | SND_ASYNC | SND_LOOP | SND_NODEFAULT,
            )
        } else {
            PlaySoundW(PCWSTR::null(), None, SND_SYNC)
        }
    };
    ensure!(
        result.as_bool(),
        "原厂静音音频会话{}失败",
        if enabled { "启动" } else { "停止" }
    );
    Ok(())
}

#[cfg(test)]
#[test]
fn the_audio_session_contains_only_silence_in_the_original_pcm_format() {
    assert_eq!(u16::from_le_bytes(SILENCE[22..24].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(SILENCE[24..28].try_into().unwrap()),
        44100
    );
    assert_eq!(u16::from_le_bytes(SILENCE[34..36].try_into().unwrap()), 16);
    let bytes = u32::from_le_bytes(SILENCE[40..44].try_into().unwrap()) as usize;
    assert_eq!(bytes, 22528 * 4);
    assert_eq!(SILENCE.len(), bytes + 44);
    assert!(SILENCE[44..].iter().all(|sample| *sample == 0));
}
