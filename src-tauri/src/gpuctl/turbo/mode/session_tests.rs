use super::*;
use crate::gpuctl::GpuController;
use std::sync::Arc;

fn setup() -> (Arc<Memory>, GpuController, Policy) {
    let memory = Arc::new(fixture());
    let policy = policy(&memory);
    let gpu = isolated_controller(memory.clone());
    gpu.set_power_suspended(true).unwrap();
    (memory, gpu, policy)
}
fn assert_suspended(gpu: &GpuController) {
    let state = gpu
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(state.power_suspended);
    assert!(state.session.is_none() && state.turbo_io.is_none() && state.gpu_name.is_none());
}

#[test]
fn confirmed_mode_control_is_short_lived_and_does_not_resume_ordinary_gpu_access() {
    let (memory, gpu, policy) = setup();
    assert!(!gpu.read_profile().available);
    assert!(gpu.read_mode_gpu_in_control().is_err());
    let prepared = gpu
        .prepare_mode_gpu_in_control(memory.as_ref(), &policy, 3, true, false, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_suspended(&gpu);
    switch_mode(&memory);
    let mut saved = None;
    gpu.apply_mode_gpu_in_control(
        memory.as_ref(),
        &policy,
        &prepared,
        &mut |write| {
            saved = Some(write.clone());
            Ok(())
        },
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(saved.as_ref().unwrap(), &prepared.write);
    assert_suspended(&gpu);
    assert!(!gpu.read_profile().available);
    let actual = gpu
        .read_confirmed_mode_gpu_in_control(&mut || Ok(()))
        .unwrap();
    assert_eq!(actual, prepared.write.after);
    assert_suspended(&gpu);
    gpu.restore_mode_gpu_in_control(&prepared.write.before, &actual, &mut || Ok(()))
        .unwrap();
    assert_eq!(
        RawState::read(memory.as_ref()).unwrap(),
        prepared.write.before
    );
    assert_suspended(&gpu);
}

#[test]
fn revoked_access_does_not_reconnect_a_suspended_gpu() {
    let (memory, gpu, policy) = setup();
    assert!(
        gpu.prepare_mode_gpu_in_control(memory.as_ref(), &policy, 3, true, false, &mut || bail!(
            "隔离许可撤销"
        ))
        .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    assert_suspended(&gpu);
    let prepared = gpu
        .prepare_mode_gpu_in_control(memory.as_ref(), &policy, 3, true, false, &mut || Ok(()))
        .unwrap()
        .unwrap();
    let reads = memory.0.lock().unwrap().reads;
    assert!(
        gpu.apply_mode_gpu_in_control(
            memory.as_ref(),
            &policy,
            &prepared,
            &mut |_| panic!("撤销后不得保存写入记录"),
            &mut || bail!("隔离撤销")
        )
        .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().reads, reads);
    assert_suspended(&gpu);
}

#[test]
fn failed_mode_preparation_also_discards_the_temporary_connection() {
    let (memory, gpu, policy) = setup();
    memory.0.lock().unwrap().fail_lock_read_at = Some(1);
    assert!(
        gpu.prepare_mode_gpu_in_control(memory.as_ref(), &policy, 3, true, false, &mut || Ok(()))
            .is_err()
    );
    assert_suspended(&gpu);
    assert!(!gpu.read_profile().available);
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn isolated_control_without_an_injected_provider_never_connects_to_native_gpu() {
    let memory = fixture();
    let policy = policy(&memory);
    let gpu = GpuController::new();
    gpu.set_power_suspended(true).unwrap();
    let error = gpu
        .prepare_mode_gpu_in_control(&memory, &policy, 3, true, false, &mut || Ok(()))
        .err()
        .unwrap();
    assert!(error.to_string().contains("未注入"));
    assert_suspended(&gpu);
}

#[test]
fn unwinding_a_confirmed_source_releases_the_temporary_connection() {
    struct Panics;
    impl SourceIo for Panics {
        fn read(&self) -> Result<Source> {
            panic!("隔离来源异常");
        }
    }
    let (_memory, gpu, policy) = setup();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ =
                gpu.prepare_mode_gpu_in_control(&Panics, &policy, 3, true, false, &mut || Ok(()));
        }))
        .is_err()
    );
    assert_suspended(&gpu);
}
