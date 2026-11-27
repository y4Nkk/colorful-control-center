use super::*;
use std::cell::Cell;

#[test]
fn only_unsupported_fields_are_disabled_for_the_current_device() {
    let mut state = Connection::default();
    state.bind(100);
    let original = state.id;
    let calls = Cell::new(0);
    let unsupported = || -> Result<u32, NvmlError> {
        calls.set(calls.get() + 1);
        Err(NvmlError::NotSupported)
    };
    for _ in 0..2 {
        assert_eq!(
            state.support.read(Query::Encoder, unsupported).unwrap(),
            None
        );
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(
        state
            .support
            .read(Query::Decoder, || Err::<u32, _>(NvmlError::Timeout))
            .unwrap(),
        None
    );
    assert_eq!(
        state.support.read(Query::Decoder, || Ok(0)).unwrap(),
        Some(0)
    );
    assert!(matches!(
        state
            .support
            .read(Query::Memory, || Err::<u32, _>(NvmlError::GpuLost)),
        Err(NvmlError::GpuLost)
    ));
    state.bind(100);
    assert_eq!(state.id, original);
    assert_eq!(
        state.support.read(Query::Encoder, unsupported).unwrap(),
        None
    );
    state.bind(101);
    assert_ne!(state.id, original);
    assert_eq!(
        state.support.read(Query::Encoder, || Ok(42)).unwrap(),
        Some(42)
    );
}

#[test]
fn valid_static_fields_are_cached_and_discarded_when_the_device_changes() {
    let mut state = Connection::default();
    state.bind(100);
    for value in [4096, 2048] {
        assert_eq!(
            state
                .support
                .static_value(&mut state.cuda_cores, Query::CudaCores, || Ok(value))
                .unwrap(),
            Some(4096),
        );
    }
    state.bind(101);
    assert_eq!(state.cuda_cores, None);
}
